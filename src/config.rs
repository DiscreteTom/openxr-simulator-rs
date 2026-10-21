use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub const APP_DIR_NAME: &str = "OpenXR-Simulator-RS";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    #[default]
    Stereo,
    LeftEye,
    RightEye,
}

impl ViewMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stereo => "Both eyes",
            Self::LeftEye => "Left eye",
            Self::RightEye => "Right eye",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Stereo => Self::LeftEye,
            Self::LeftEye => Self::RightEye,
            Self::RightEye => Self::Stereo,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplaySettings {
    pub view_mode: ViewMode,
    pub window_width: u32,
    pub window_height: u32,
    pub mirror_rate_hz: u32,
    pub render_width: u32,
    pub render_height: u32,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            view_mode: ViewMode::Stereo,
            window_width: 1280,
            window_height: 720,
            mirror_rate_hz: 90,
            render_width: 1280,
            render_height: 1408,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FovSettings {
    pub horizontal_degrees: f32,
    pub vertical_degrees: f32,
    pub ipd_meters: f32,
}

impl Default for FovSettings {
    fn default() -> Self {
        Self {
            horizontal_degrees: 100.0,
            vertical_degrees: 96.0,
            ipd_meters: 0.064,
        }
    }
}

impl FovSettings {
    pub fn sanitize(&mut self) {
        self.horizontal_degrees = self.horizontal_degrees.clamp(40.0, 150.0);
        self.vertical_degrees = self.vertical_degrees.clamp(30.0, 130.0);
        self.ipd_meters = self.ipd_meters.clamp(0.0, 0.12);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MovementSettings {
    pub speed_meters_per_second: f32,
    pub fast_multiplier: f32,
    pub mouse_sensitivity_degrees_per_pixel: f32,
    pub invert_y: bool,
    pub capture_mouse_on_start: bool,
}

impl Default for MovementSettings {
    fn default() -> Self {
        Self {
            speed_meters_per_second: 3.0,
            fast_multiplier: 4.0,
            mouse_sensitivity_degrees_per_pixel: 0.12,
            invert_y: false,
            capture_mouse_on_start: true,
        }
    }
}

impl MovementSettings {
    pub fn sanitize(&mut self) {
        self.speed_meters_per_second = self.speed_meters_per_second.clamp(0.1, 30.0);
        self.fast_multiplier = self.fast_multiplier.clamp(1.0, 20.0);
        self.mouse_sensitivity_degrees_per_pixel =
            self.mouse_sensitivity_degrees_per_pixel.clamp(0.01, 2.0);
    }
}

/// Key names are case-insensitive and accept aliases such as `Space`, `LShift`,
/// `Left`, `MouseLeft`, and `F1`. A single character binds that key.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyBindings {
    pub forward: String,
    pub backward: String,
    pub strafe_left: String,
    pub strafe_right: String,
    pub up: String,
    pub down: String,
    pub fast: String,
    pub reset_pose: String,
    pub cycle_eye: String,
    pub reload_config: String,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            forward: "W".into(),
            backward: "S".into(),
            strafe_left: "A".into(),
            strafe_right: "D".into(),
            up: "Space".into(),
            down: "LCtrl".into(),
            fast: "CapsLock".into(),
            reset_pose: "R".into(),
            cycle_eye: "F6".into(),
            reload_config: "F5".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ControllerBindings {
    pub trigger: String,
    pub grip: String,
    pub menu: String,
    pub system: String,
    pub primary: String,
    pub secondary: String,
    pub stick_click: String,
    pub stick_up: String,
    pub stick_down: String,
    pub stick_left: String,
    pub stick_right: String,
}

impl ControllerBindings {
    pub fn left() -> Self {
        Self {
            trigger: "F".into(),
            grip: "G".into(),
            menu: "M".into(),
            system: "T".into(),
            primary: "Z".into(),
            secondary: "X".into(),
            stick_click: "V".into(),
            stick_up: "Up".into(),
            stick_down: "Down".into(),
            stick_left: "Left".into(),
            stick_right: "Right".into(),
        }
    }

    pub fn right() -> Self {
        Self {
            trigger: "J".into(),
            grip: "K".into(),
            menu: "N".into(),
            system: "Y".into(),
            primary: "B".into(),
            secondary: "H".into(),
            stick_click: "L".into(),
            stick_up: "Numpad8".into(),
            stick_down: "Numpad2".into(),
            stick_left: "Numpad4".into(),
            stick_right: "Numpad6".into(),
        }
    }
}

impl Default for ControllerBindings {
    fn default() -> Self {
        Self::left()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ControllerSettings {
    pub left_modifier: String,
    pub right_modifier: String,
    pub move_speed_meters_per_second: f32,
    pub mouse_sensitivity_degrees_per_pixel: f32,
    pub invert_y: bool,
    pub left: ControllerBindings,
    pub right: ControllerBindings,
}

impl Default for ControllerSettings {
    fn default() -> Self {
        Self {
            left_modifier: "LShift".into(),
            right_modifier: "RShift".into(),
            move_speed_meters_per_second: 1.0,
            mouse_sensitivity_degrees_per_pixel: 0.18,
            invert_y: false,
            left: ControllerBindings::left(),
            right: ControllerBindings::right(),
        }
    }
}

impl ControllerSettings {
    pub fn sanitize(&mut self) {
        self.move_speed_meters_per_second = self.move_speed_meters_per_second.clamp(0.1, 10.0);
        self.mouse_sensitivity_degrees_per_pixel =
            self.mouse_sensitivity_degrees_per_pixel.clamp(0.01, 2.0);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub display: DisplaySettings,
    pub fov: FovSettings,
    pub movement: MovementSettings,
    pub keys: KeyBindings,
    pub controller: ControllerSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            display: DisplaySettings::default(),
            fov: FovSettings::default(),
            movement: MovementSettings::default(),
            keys: KeyBindings::default(),
            controller: ControllerSettings::default(),
        }
    }
}

impl Settings {
    pub fn sanitize(&mut self) {
        self.display.render_width = self.display.render_width.clamp(256, 4096);
        self.display.render_height = self.display.render_height.clamp(256, 4096);
        self.display.window_width = self.display.window_width.clamp(480, 7680);
        self.display.window_height = self.display.window_height.clamp(320, 4320);
        self.display.mirror_rate_hz = match self.display.mirror_rate_hz {
            0 | 15 | 30 | 60 | 90 | 120 | 144 => self.display.mirror_rate_hz,
            _ => 90,
        };
        self.fov.sanitize();
        self.movement.sanitize();
        self.controller.sanitize();
        let left_modifier = virtual_key(&self.controller.left_modifier);
        let right_modifier = virtual_key(&self.controller.right_modifier);
        let fast = virtual_key(&self.keys.fast);
        let up = virtual_key(&self.keys.up);
        let down = virtual_key(&self.keys.down);
        if fast.is_some()
            && (fast == left_modifier || fast == right_modifier || fast == up || fast == down)
        {
            self.keys.fast = "CapsLock".into();
        }
    }
}

pub fn data_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("LOCALAPPDATA") {
        PathBuf::from(path).join(APP_DIR_NAME)
    } else {
        PathBuf::from(".").join(APP_DIR_NAME)
    }
}

pub fn config_path() -> PathBuf {
    std::env::var_os("OPENXR_SIMULATOR_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir().join("settings.json"))
}

pub fn load_from(path: &Path) -> Settings {
    let mut settings = fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
        .unwrap_or_default();
    settings.sanitize();
    settings
}

pub fn save_to(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(settings)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, text)?;
    fs::rename(temporary, path)
}

static SETTINGS: OnceLock<Mutex<Settings>> = OnceLock::new();

pub fn init() {
    let mut settings = load_from(&config_path());
    settings.sanitize();
    let path = config_path();
    if !path.exists() {
        let _ = save_to(&path, &settings);
    }
    SETTINGS.get_or_init(|| Mutex::new(settings));
}

pub fn snapshot() -> Settings {
    SETTINGS
        .get()
        .map(|lock| lock.lock().unwrap_or_else(|e| e.into_inner()).clone())
        .unwrap_or_default()
}

pub fn update<R>(f: impl FnOnce(&mut Settings) -> R) -> R {
    init();
    let mut guard = SETTINGS
        .get()
        .expect("settings initialized")
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let result = f(&mut guard);
    guard.sanitize();
    result
}

pub fn reload() -> Settings {
    let settings = load_from(&config_path());
    SETTINGS
        .get()
        .expect("settings initialized")
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone_from(&settings);
    settings
}

pub fn persist() {
    init();
    let settings = snapshot();
    if let Err(error) = save_to(&config_path(), &settings) {
        crate::logger::error(format_args!("failed to save settings: {error}"));
    }
}

/// Returns a Win32 virtual-key code, or `None` for an unknown key name.
pub fn virtual_key(name: &str) -> Option<u16> {
    let normalized: String = name
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && *c != '_' && *c != '-')
        .flat_map(char::to_lowercase)
        .collect();

    let single = normalized.as_bytes();
    if single.len() == 1 {
        let byte = single[0].to_ascii_uppercase();
        if byte.is_ascii_alphanumeric() {
            return Some(byte as u16);
        }
        let punctuation = match byte {
            b';' => 0xBA,
            b'=' => 0xBB,
            b',' => 0xBC,
            b'-' => 0xBD,
            b'.' => 0xBE,
            b'/' => 0xBF,
            b'`' => 0xC0,
            b'[' => 0xDB,
            b'\\' => 0xDC,
            b']' => 0xDD,
            b'\'' => 0xDE,
            _ => 0,
        };
        if punctuation != 0 {
            return Some(punctuation);
        }
    }

    let value = match normalized.as_str() {
        "space" => 0x20,
        "tab" => 0x09,
        "enter" | "return" => 0x0D,
        "escape" | "esc" => 0x1B,
        "backspace" => 0x08,
        "insert" => 0x2D,
        "delete" | "del" => 0x2E,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" | "pgup" => 0x21,
        "pagedown" | "pgdn" => 0x22,
        "left" | "arrowleft" => 0x25,
        "up" | "arrowup" => 0x26,
        "right" | "arrowright" => 0x27,
        "down" | "arrowdown" => 0x28,
        "shift" | "lshift" | "leftshift" => 0xA0,
        "rshift" | "rightshift" => 0xA1,
        "ctrl" | "control" | "lctrl" | "leftctrl" => 0xA2,
        "rctrl" | "rightctrl" => 0xA3,
        "alt" | "lalt" | "leftalt" => 0xA4,
        "ralt" | "rightalt" => 0xA5,
        "mouseleft" | "lmb" => 0x01,
        "mouseright" | "rmb" => 0x02,
        "mousemiddle" | "mmb" => 0x04,
        "numpad0" => 0x60,
        "numpad1" => 0x61,
        "numpad2" => 0x62,
        "numpad3" => 0x63,
        "numpad4" => 0x64,
        "numpad5" => 0x65,
        "numpad6" => 0x66,
        "numpad7" => 0x67,
        "numpad8" => 0x68,
        "numpad9" => 0x69,
        "numpadmultiply" => 0x6A,
        "numpadadd" => 0x6B,
        "numpadsubtract" => 0x6D,
        "numpaddecimal" => 0x6E,
        "numpaddivide" => 0x6F,
        "capslock" => 0x14,
        _ if normalized.starts_with('f') => {
            let number = normalized[1..].parse::<u16>().ok()?;
            if (1..=24).contains(&number) {
                0x6F + number
            } else {
                return None;
            }
        }
        _ => return None,
    };
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_keys() {
        assert_eq!(virtual_key("w"), Some(0x57));
        assert_eq!(virtual_key("Left Shift"), Some(0xA0));
        assert_eq!(virtual_key("F12"), Some(0x7B));
        assert_eq!(virtual_key("Numpad8"), Some(0x68));
        assert_eq!(virtual_key("CapsLock"), Some(0x14));
        assert_eq!(virtual_key("not-a-key"), None);
    }

    #[test]
    fn defaults_are_sane() {
        let mut settings = Settings::default();
        settings.display.render_width = 10;
        settings.fov.horizontal_degrees = 999.0;
        settings.sanitize();
        assert_eq!(settings.display.render_width, 256);
        assert_eq!(settings.fov.horizontal_degrees, 150.0);
    }

    #[test]
    fn movement_speed_key_cannot_shadow_controller_modifiers() {
        let mut settings = Settings::default();
        settings.controller.left_modifier = "F8".into();
        settings.keys.fast = "F8".into();
        settings.sanitize();
        assert_eq!(settings.keys.fast, "CapsLock");
    }
}
