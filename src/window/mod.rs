use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, Ordering};
use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, CreateSolidBrush, DIB_RGB_COLORS, DrawTextW,
    EndPaint, FillRect, InvalidateRect, PAINTSTRUCT, SRCCOPY, SetBkMode, SetTextColor,
    StretchDIBits, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE, VK_F5};
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRect, AppendMenuW, CheckMenuRadioItem, CreateMenu, CreateWindowExW, DefWindowProcW,
    DestroyWindow, DispatchMessageW, GetClientRect, GetCursorPos, GetForegroundWindow, IDC_ARROW,
    IsWindow, LoadCursorW, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING, MSG, PM_REMOVE,
    PeekMessageW, RegisterClassW, SW_SHOW, SetCursor, SetCursorPos, SetForegroundWindow,
    ShowWindow, TranslateMessage, UnregisterClassW, WINDOW_EX_STYLE, WM_ACTIVATE, WM_ACTIVATEAPP,
    WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_KEYDOWN, WM_KILLFOCUS, WM_LBUTTONDOWN, WM_PAINT,
    WM_RBUTTONDOWN, WM_SETCURSOR, WM_SIZE, WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

use crate::config::{self, ViewMode};

mod menu;
mod message;
mod paint;

use menu::{build_menu, sync_menu_checks};
use message::wnd_proc;

const WINDOW_CLASS: PCWSTR = w!("OpenXRSimulatorRustPreview");
const ID_VIEW_STEREO: u32 = 1001;
const ID_VIEW_LEFT: u32 = 1002;
const ID_VIEW_RIGHT: u32 = 1003;
const ID_FOV_DECREASE: u32 = 1010;
const ID_FOV_INCREASE: u32 = 1011;
const ID_FOV_RESET: u32 = 1012;
const ID_IPD_DECREASE: u32 = 1013;
const ID_IPD_INCREASE: u32 = 1014;
const ID_VFOV_DECREASE: u32 = 1015;
const ID_VFOV_INCREASE: u32 = 1016;
const ID_CAPTURE_MOUSE: u32 = 1020;
const ID_RESET_POSE: u32 = 1021;
const ID_RESET_VIEW: u32 = 1022;
const ID_RELOAD_CONFIG: u32 = 1023;
const ID_OPEN_CONFIG_FILE: u32 = 1024;

static WINDOW_HANDLE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static MENU_HANDLE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static CLASS_REGISTERED: AtomicBool = AtomicBool::new(false);
static MOUSE_CAPTURED: AtomicBool = AtomicBool::new(false);
static REQUEST_EXIT: AtomicBool = AtomicBool::new(false);
static RESET_POSE_REQUESTED: AtomicBool = AtomicBool::new(false);
static CLIENT_WIDTH: AtomicI32 = AtomicI32::new(1280);
static CLIENT_HEIGHT: AtomicI32 = AtomicI32::new(720);

static FRAME: OnceLock<Mutex<Option<FrameImage>>> = OnceLock::new();

#[derive(Clone)]
pub struct FrameImage {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

fn frame_slot() -> &'static Mutex<Option<FrameImage>> {
    FRAME.get_or_init(|| Mutex::new(None))
}

pub fn hwnd() -> HWND {
    HWND(WINDOW_HANDLE.load(Ordering::Acquire))
}

pub fn is_open() -> bool {
    let handle = hwnd();
    !handle.is_invalid() && unsafe { IsWindow(Some(handle)).as_bool() }
}

pub fn request_exit() -> bool {
    REQUEST_EXIT.swap(false, Ordering::AcqRel)
}

pub fn take_reset_pose_request() -> bool {
    RESET_POSE_REQUESTED.swap(false, Ordering::AcqRel)
}

pub fn mouse_captured() -> bool {
    MOUSE_CAPTURED.load(Ordering::Acquire)
}

pub fn set_mouse_capture(captured: bool) {
    MOUSE_CAPTURED.store(captured, Ordering::Release);
    if captured {
        center_cursor();
    }
}

fn center_cursor() {
    let handle = hwnd();
    if handle.is_invalid() {
        return;
    }
    unsafe {
        let mut rect = RECT::default();
        if GetClientRect(handle, &mut rect).is_ok() {
            let mut point = POINT::default();
            if GetCursorPos(&mut point).is_ok() {
                let center_x = (rect.right - rect.left) / 2;
                let center_y = (rect.bottom - rect.top) / 2;
                let mut origin = POINT::default();
                let _ = windows::Win32::Graphics::Gdi::ClientToScreen(
                    handle,
                    &mut origin as *mut POINT,
                );
                let _ = SetCursorPos(origin.x + center_x, origin.y + center_y);
            }
        }
    }
}

/// Returns `(dx, dy)` since the previous call and re-centres the cursor while
/// mouse capture is enabled. The first sample after capture is always zero.
pub fn consume_mouse_delta() -> (f32, f32) {
    let handle = hwnd();
    if handle.is_invalid() || !mouse_captured() {
        return (0.0, 0.0);
    }
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground != handle {
            // The host game often owns the foreground window. Accept input when
            // any window in this process is active.
            let mut pid = 0u32;
            windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                foreground,
                Some(&mut pid),
            );
            if pid != std::process::id() {
                set_mouse_capture(false);
                return (0.0, 0.0);
            }
        }

        let mut cursor = POINT::default();
        if GetCursorPos(&mut cursor).is_err() {
            return (0.0, 0.0);
        }
        let mut rect = RECT::default();
        if GetClientRect(handle, &mut rect).is_err() {
            return (0.0, 0.0);
        }
        let mut origin = POINT::default();
        if !windows::Win32::Graphics::Gdi::ClientToScreen(handle, &mut origin as *mut POINT)
            .as_bool()
        {
            return (0.0, 0.0);
        }
        let center_x = origin.x + (rect.right - rect.left) / 2;
        let center_y = origin.y + (rect.bottom - rect.top) / 2;
        let dx = (cursor.x - center_x) as f32;
        let dy = (cursor.y - center_y) as f32;
        let _ = SetCursorPos(center_x, center_y);
        (dx, dy)
    }
}

pub fn create() -> Result<(), String> {
    if is_open() {
        return Ok(());
    }
    REQUEST_EXIT.store(false, Ordering::Release);

    unsafe {
        if !CLASS_REGISTERED.swap(true, Ordering::AcqRel) {
            let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(wnd_proc),
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: WINDOW_CLASS,
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 {
                CLASS_REGISTERED.store(false, Ordering::Release);
                return Err(format!(
                    "RegisterClassW failed: {}",
                    windows::core::Error::from_thread()
                ));
            }
        }

        let menu = build_menu()?;
        let settings = config::snapshot();
        let content_width = settings.display.window_width as i32;
        let content_height = settings.display.window_height as i32;
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: content_width,
            bottom: content_height,
        };
        let _ = AdjustWindowRect(&mut rect, WS_OVERLAPPEDWINDOW, true);
        let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let handle = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            WINDOW_CLASS,
            w!("OpenXR Simulator - waiting for an XR frame"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            100,
            80,
            rect.right - rect.left,
            rect.bottom - rect.top,
            None,
            Some(menu),
            Some(instance.into()),
            None,
        )
        .map_err(|e| format!("CreateWindowExW failed: {e}"))?;

        WINDOW_HANDLE.store(handle.0, Ordering::Release);
        MENU_HANDLE.store(menu.0, Ordering::Release);
        let _ = ShowWindow(handle, SW_SHOW);
        let _ = SetForegroundWindow(handle);
        sync_menu_checks();
        set_mouse_capture(settings.movement.capture_mouse_on_start);
    }
    Ok(())
}

pub fn destroy() {
    let handle = hwnd();
    if handle.is_invalid() {
        return;
    }
    set_mouse_capture(false);
    WINDOW_HANDLE.store(ptr::null_mut(), Ordering::Release);
    unsafe {
        let _ = DestroyWindow(handle);
        if CLASS_REGISTERED.swap(false, Ordering::AcqRel)
            && let Ok(instance) = GetModuleHandleW(None)
        {
            let _ = UnregisterClassW(WINDOW_CLASS, Some(instance.into()));
        }
    }
}

pub fn pump_messages() {
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

pub fn publish(frame: FrameImage) {
    if let Ok(mut slot) = frame_slot().lock() {
        *slot = Some(frame);
    }
    let handle = hwnd();
    if !handle.is_invalid() {
        unsafe {
            let _ = InvalidateRect(Some(handle), None, false);
        }
    }
}

pub fn publish_composite(eyes: &[(&[u8], u32, u32)], mode: ViewMode) {
    let picked: Vec<&(&[u8], u32, u32)> = match mode {
        ViewMode::Stereo => eyes.iter().collect(),
        ViewMode::LeftEye => eyes.first().into_iter().collect(),
        ViewMode::RightEye => eyes.get(1).into_iter().collect(),
    };
    if picked.is_empty() {
        return;
    }

    let height = picked.iter().map(|eye| eye.2).max().unwrap_or(1);
    let width: u32 = picked.iter().map(|eye| eye.1).sum();
    let mut bgra = vec![0u8; width as usize * height as usize * 4];
    let mut offset_x = 0u32;
    for (pixels, eye_width, eye_height) in picked {
        for row in 0..(*eye_height).min(height) {
            let source = row as usize * *eye_width as usize * 4;
            let destination = (row as usize * width as usize + offset_x as usize) * 4;
            let count = (*eye_width as usize * 4).min(bgra.len().saturating_sub(destination));
            if source + count <= pixels.len() {
                bgra[destination..destination + count]
                    .copy_from_slice(&pixels[source..source + count]);
            }
        }
        offset_x = offset_x.saturating_add(*eye_width);
    }
    publish(FrameImage {
        width,
        height,
        bgra,
    });
}

pub fn update_title(frame_fps: f32, mode: ViewMode, hfov: f32, vfov: f32, ipd: f32) {
    let handle = hwnd();
    if handle.is_invalid() {
        return;
    }
    let title = format!(
        "OpenXR Simulator  |  {:.1} FPS  |  {}  |  FOV {:.0} x {:.0}  |  IPD {:.1} mm  |  Esc releases mouse",
        frame_fps,
        mode.label(),
        hfov,
        vfov,
        ipd * 1000.0
    );
    let wide: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let _ =
            windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(handle, PCWSTR(wide.as_ptr()));
    }
}

pub fn key_down(virtual_key: u16) -> bool {
    unsafe { GetAsyncKeyState(virtual_key as i32) & 0x8000u16 as i16 != 0 }
}

pub fn client_size() -> (u32, u32) {
    let width = CLIENT_WIDTH.load(Ordering::Acquire).max(1) as u32;
    let height = CLIENT_HEIGHT.load(Ordering::Acquire).max(1) as u32;
    (width, height)
}
