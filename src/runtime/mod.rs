#![allow(non_snake_case)]

use std::collections::{HashMap, VecDeque};
use std::ffi::c_char;
use std::hash::{Hash, Hasher};
use std::ptr;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use openxr_sys as xr;
use openxr_sys::Handle;

use crate::config;
use crate::d3d11;
use crate::d3d11_preview;
use crate::math::{self, Vec3};
use crate::window;

mod actions;
mod dispatch;
mod instance;
mod presentation;
mod session;
mod spaces;
mod utility;

pub use instance::xrNegotiateLoaderRuntimeInterface;

const EXTENSIONS: &[&str] = &[
    "XR_KHR_D3D11_enable",
    "XR_KHR_win32_convert_performance_counter_time",
    "XR_KHR_composition_layer_depth",
];

const SUCCESS: xr::Result = xr::Result::SUCCESS;
const VALIDATION_FAILURE: xr::Result = xr::Result::ERROR_VALIDATION_FAILURE;
const FUNCTION_UNSUPPORTED: xr::Result = xr::Result::ERROR_FUNCTION_UNSUPPORTED;
const HANDLE_INVALID: xr::Result = xr::Result::ERROR_HANDLE_INVALID;
const SIZE_INSUFFICIENT: xr::Result = xr::Result::ERROR_SIZE_INSUFFICIENT;
const EXTENSION_NOT_PRESENT: xr::Result = xr::Result::ERROR_EXTENSION_NOT_PRESENT;
const SYSTEM_INVALID: xr::Result = xr::Result::ERROR_SYSTEM_INVALID;
const FORM_FACTOR_UNSUPPORTED: xr::Result = xr::Result::ERROR_FORM_FACTOR_UNSUPPORTED;
const PATH_INVALID: xr::Result = xr::Result::ERROR_PATH_INVALID;
const RUNTIME_FAILURE: xr::Result = xr::Result::ERROR_RUNTIME_FAILURE;
const GRAPHICS_DEVICE_INVALID: xr::Result = xr::Result::ERROR_GRAPHICS_DEVICE_INVALID;
const EVENT_UNAVAILABLE: xr::Result = xr::Result::EVENT_UNAVAILABLE;

static START: OnceLock<Instant> = OnceLock::new();
static RUNTIME: OnceLock<Mutex<RuntimeState>> = OnceLock::new();

fn runtime() -> &'static Mutex<RuntimeState> {
    RUNTIME.get_or_init(|| Mutex::new(RuntimeState::new()))
}

fn now_ns() -> i64 {
    START
        .get_or_init(Instant::now)
        .elapsed()
        .as_nanos()
        .min(i64::MAX as u128) as i64
}

#[derive(Clone)]
struct HeadState {
    position: Vec3,
    yaw: f32,
    pitch: f32,
    last_input: Instant,
    previous_cycle_eye: bool,
    previous_reload: bool,
}

impl Default for HeadState {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 1.7, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            last_input: Instant::now(),
            previous_cycle_eye: false,
            previous_reload: false,
        }
    }
}

struct SessionData {
    handle: xr::Session,
    state: xr::SessionState,
    mirror: d3d11::Mirror,
    preview: Option<d3d11_preview::Preview>,
    preview_attempted: bool,
}

struct SwapchainData {
    images: Vec<windows::Win32::Graphics::Direct3D11::ID3D11Texture2D>,
    next_image: usize,
    acquired: Option<u32>,
    last_released: Option<u32>,
    is_depth: bool,
}

#[derive(Debug, Clone, Copy)]
enum SpaceKind {
    View,
    Local,
    Stage,
    Controller { hand: u8, pose: xr::Posef },
}

#[derive(Debug, Clone, Copy)]
struct SpaceData {
    kind: SpaceKind,
}

#[derive(Debug, Clone, Copy)]
enum RuntimeEvent {
    SessionStateChanged {
        session: xr::Session,
        state: xr::SessionState,
    },
    InteractionProfileChanged {
        session: xr::Session,
    },
}

#[derive(Debug)]
struct ActionData {
    name: String,
}

struct RuntimeState {
    instance: Option<xr::Instance>,
    session: Option<SessionData>,
    next_handle: u64,
    swapchains: HashMap<u64, SwapchainData>,
    spaces: HashMap<u64, SpaceData>,
    events: VecDeque<RuntimeEvent>,
    paths: HashMap<u64, String>,
    path_reverse: HashMap<String, u64>,
    actions: HashMap<u64, ActionData>,
    action_sets: HashMap<u64, String>,
    suggested_profiles: Vec<xr::Path>,
    active_profile: xr::Path,
    head: HeadState,
    next_frame: Instant,
    last_mirror: Option<Instant>,
    fps_window_start: Instant,
    fps_frames: u32,
    fps: f32,
    last_title: Instant,
}

impl RuntimeState {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            instance: None,
            session: None,
            next_handle: 0x1000,
            swapchains: HashMap::new(),
            spaces: HashMap::new(),
            events: VecDeque::new(),
            paths: HashMap::new(),
            path_reverse: HashMap::new(),
            actions: HashMap::new(),
            action_sets: HashMap::new(),
            suggested_profiles: Vec::new(),
            active_profile: xr::Path::NULL,
            head: HeadState::default(),
            next_frame: now,
            last_mirror: None,
            fps_window_start: now,
            fps_frames: 0,
            fps: 0.0,
            last_title: now,
        }
    }

    fn allocate_handle(&mut self) -> u64 {
        let value = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1);
        value
    }

    fn push_session_state(&mut self, state: xr::SessionState) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        if session.state == state {
            return;
        }
        session.state = state;
        self.events.push_back(RuntimeEvent::SessionStateChanged {
            session: session.handle,
            state,
        });
    }

    fn reset_pose(&mut self) {
        self.head.position = Vec3::new(0.0, 1.7, 0.0);
        self.head.yaw = 0.0;
        self.head.pitch = 0.0;
    }

    fn update_input(&mut self) {
        window::pump_messages();
        if window::request_exit() && self.session.is_some() {
            self.push_session_state(xr::SessionState::EXITING);
        }
        if window::take_reset_pose_request() {
            self.reset_pose();
        }

        let settings = config::snapshot();
        let reset_key = config::virtual_key(&settings.keys.reset_pose);
        if reset_key.is_some_and(window::key_down) {
            self.reset_pose();
        }

        let cycle_eye_key = config::virtual_key(&settings.keys.cycle_eye);
        let cycle_eye = cycle_eye_key.is_some_and(window::key_down);
        if cycle_eye && !self.head.previous_cycle_eye {
            config::update(|settings| {
                settings.display.view_mode = settings.display.view_mode.next();
            });
            config::persist();
        }
        self.head.previous_cycle_eye = cycle_eye;

        let reload_key = config::virtual_key(&settings.keys.reload_config);
        let reload = reload_key.is_some_and(window::key_down);
        if reload && !self.head.previous_reload {
            config::reload();
        }
        self.head.previous_reload = reload;

        let now = Instant::now();
        let delta = (now - self.head.last_input).as_secs_f32().clamp(0.0, 0.05);
        self.head.last_input = now;

        let (dx, dy) = window::consume_mouse_delta();
        if dx != 0.0 || dy != 0.0 {
            let sensitivity = settings
                .movement
                .mouse_sensitivity_degrees_per_pixel
                .to_radians();
            self.head.yaw -= dx * sensitivity;
            let pitch_direction = if settings.movement.invert_y {
                1.0
            } else {
                -1.0
            };
            self.head.pitch += dy * sensitivity * pitch_direction;
            self.head.pitch = self
                .head
                .pitch
                .clamp(-89.0_f32.to_radians(), 89.0_f32.to_radians());
        }

        let down = |key: &str| config::virtual_key(key).is_some_and(window::key_down);
        let mut local = Vec3::default();
        if down(&settings.keys.forward) {
            local.z -= 1.0;
        }
        if down(&settings.keys.backward) {
            local.z += 1.0;
        }
        if down(&settings.keys.strafe_left) {
            local.x -= 1.0;
        }
        if down(&settings.keys.strafe_right) {
            local.x += 1.0;
        }
        if local.x != 0.0 || local.z != 0.0 {
            let length = (local.x * local.x + local.z * local.z).sqrt();
            local.x /= length;
            local.z /= length;
        }
        let rotated = math::rotate_xz(self.head.yaw, local);
        let mut speed = settings.movement.speed_meters_per_second;
        if down(&settings.keys.fast) {
            speed *= settings.movement.fast_multiplier;
        }
        self.head.position.x += rotated.x * speed * delta;
        self.head.position.z += rotated.z * speed * delta;
        if down(&settings.keys.up) {
            self.head.position.y += speed * delta;
        }
        if down(&settings.keys.down) {
            self.head.position.y -= speed * delta;
        }
    }

    fn head_pose(&self) -> xr::Posef {
        math::head_pose(self.head.yaw, self.head.pitch, self.head.position)
    }

    fn update_fps_and_title(&mut self) {
        self.fps_frames = self.fps_frames.saturating_add(1);
        let elapsed = self.fps_window_start.elapsed();
        if elapsed >= Duration::from_millis(500) {
            self.fps = self.fps_frames as f32 / elapsed.as_secs_f32();
            self.fps_frames = 0;
            self.fps_window_start = Instant::now();
        }
        if self.last_title.elapsed() >= Duration::from_millis(500) {
            let settings = config::snapshot();
            window::update_title(
                self.fps,
                settings.display.view_mode,
                settings.fov.horizontal_degrees,
                settings.fov.vertical_degrees,
                settings.fov.ipd_meters,
            );
            self.last_title = Instant::now();
        }
    }
}

fn lock_runtime() -> std::sync::MutexGuard<'static, RuntimeState> {
    runtime().lock().unwrap_or_else(|e| e.into_inner())
}

fn fixed_string(value: &[c_char]) -> String {
    let length = value.iter().position(|c| *c == 0).unwrap_or(value.len());
    let bytes: Vec<u8> = value[..length].iter().map(|c| *c as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

unsafe fn copy_chars<const N: usize>(target: &mut [c_char; N], value: &str) {
    target.fill(0);
    for (destination, source) in target
        .iter_mut()
        .zip(value.as_bytes().iter().copied().take(N.saturating_sub(1)))
    {
        *destination = source as c_char;
    }
}

unsafe fn write_count_buffer<T: Copy>(
    capacity: u32,
    count: *mut u32,
    values: &[T],
    output: *mut T,
) -> xr::Result {
    if !count.is_null() {
        unsafe { *count = values.len() as u32 };
    }
    if capacity == 0 {
        return SUCCESS;
    }
    if output.is_null() {
        return VALIDATION_FAILURE;
    }
    if (capacity as usize) < values.len() {
        return SIZE_INSUFFICIENT;
    }
    let count_to_copy = (capacity as usize).min(values.len());
    unsafe { ptr::copy_nonoverlapping(values.as_ptr(), output, count_to_copy) };
    SUCCESS
}

fn path_id(path: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish().max(1)
}

fn action_name_matches(name: &str, pattern: &str) -> bool {
    name.to_ascii_lowercase().contains(pattern)
}
