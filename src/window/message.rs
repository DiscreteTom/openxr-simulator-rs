use super::menu::handle_command;
use super::paint::paint_frame;
use super::*;

const MIN_WINDOW_WIDTH: i32 = 480;
const MIN_WINDOW_HEIGHT: i32 = 320;

#[derive(Clone, Copy)]
struct ResizeEdge {
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
}

#[derive(Clone, Copy)]
struct ResizeState {
    edge: ResizeEdge,
    cursor: POINT,
    window: RECT,
}

static RESIZE_STATE: OnceLock<Mutex<Option<ResizeState>>> = OnceLock::new();

fn resize_state() -> &'static Mutex<Option<ResizeState>> {
    RESIZE_STATE.get_or_init(|| Mutex::new(None))
}

fn edge_for_hit(hit: u32) -> Option<ResizeEdge> {
    let edge = match hit {
        value if value == HTLEFT => ResizeEdge {
            left: true,
            right: false,
            top: false,
            bottom: false,
        },
        value if value == HTRIGHT => ResizeEdge {
            left: false,
            right: true,
            top: false,
            bottom: false,
        },
        value if value == HTTOP => ResizeEdge {
            left: false,
            right: false,
            top: true,
            bottom: false,
        },
        value if value == HTBOTTOM => ResizeEdge {
            left: false,
            right: false,
            top: false,
            bottom: true,
        },
        value if value == HTTOPLEFT => ResizeEdge {
            left: true,
            right: false,
            top: true,
            bottom: false,
        },
        value if value == HTTOPRIGHT => ResizeEdge {
            left: false,
            right: true,
            top: true,
            bottom: false,
        },
        value if value == HTBOTTOMLEFT => ResizeEdge {
            left: true,
            right: false,
            top: false,
            bottom: true,
        },
        value if value == HTBOTTOMRIGHT => ResizeEdge {
            left: false,
            right: true,
            top: false,
            bottom: true,
        },
        _ => return None,
    };
    Some(edge)
}

fn begin_resize(hwnd: HWND, edge: ResizeEdge) {
    let mut cursor = POINT::default();
    let mut window = RECT::default();
    if unsafe { GetCursorPos(&mut cursor) }.is_err()
        || unsafe { GetWindowRect(hwnd, &mut window) }.is_err()
    {
        return;
    }
    unsafe {
        SetCapture(hwnd);
    }
    if let Ok(mut state) = resize_state().lock() {
        *state = Some(ResizeState {
            edge,
            cursor,
            window,
        });
    }
}

fn update_resize(hwnd: HWND) {
    let mut cursor = POINT::default();
    if unsafe { GetCursorPos(&mut cursor) }.is_err() {
        return;
    }
    let Ok(state) = resize_state().lock() else {
        return;
    };
    let Some(resize) = state.as_ref() else {
        return;
    };
    let rect = resized_window_rect(
        resize.window,
        resize.edge,
        cursor.x - resize.cursor.x,
        cursor.y - resize.cursor.y,
    );
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            None,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

fn finish_resize() {
    if let Ok(mut state) = resize_state().lock() {
        *state = None;
    }
    unsafe {
        let _ = ReleaseCapture();
    }
}

fn resizing() -> bool {
    resize_state()
        .lock()
        .map(|state| state.is_some())
        .unwrap_or(false)
}

fn resized_window_rect(start: RECT, edge: ResizeEdge, dx: i32, dy: i32) -> RECT {
    let mut result = start;
    if edge.left {
        result.left = (start.left + dx).min(start.right - MIN_WINDOW_WIDTH);
    }
    if edge.right {
        result.right = (start.right + dx).max(start.left + MIN_WINDOW_WIDTH);
    }
    if edge.top {
        result.top = (start.top + dy).min(start.bottom - MIN_WINDOW_HEIGHT);
    }
    if edge.bottom {
        result.bottom = (start.bottom + dy).max(start.top + MIN_WINDOW_HEIGHT);
    }
    result
}

fn suppress_ime_ui(lparam: isize) -> isize {
    lparam & !(ISC_SHOWUICOMPOSITIONWINDOW as isize)
}

pub(super) unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message {
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut paint);
                paint_frame(dc);
                let _ = EndPaint(hwnd, &paint);
                LRESULT(0)
            }
            WM_SIZE => {
                CLIENT_WIDTH.store((lparam.0 & 0xffff) as i32, Ordering::Release);
                CLIENT_HEIGHT.store(((lparam.0 >> 16) & 0xffff) as i32, Ordering::Release);
                let _ = InvalidateRect(Some(hwnd), None, false);
                LRESULT(0)
            }
            WM_COMMAND => {
                handle_command((wparam.0 & 0xffff) as u32);
                LRESULT(0)
            }
            WM_NCLBUTTONDOWN => {
                if let Some(edge) = edge_for_hit(wparam.0 as u32) {
                    begin_resize(hwnd, edge);
                    LRESULT(0)
                } else {
                    DefWindowProcW(hwnd, message, wparam, lparam)
                }
            }
            WM_MOUSEMOVE if resizing() => {
                update_resize(hwnd);
                LRESULT(0)
            }
            WM_LBUTTONUP if resizing() => {
                finish_resize();
                LRESULT(0)
            }
            WM_CAPTURECHANGED if resizing() => {
                finish_resize();
                LRESULT(0)
            }
            WM_LBUTTONDOWN | WM_RBUTTONDOWN => {
                if !mouse_captured() {
                    set_mouse_capture(true);
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if wparam.0 == VK_ESCAPE.0 as usize {
                    set_mouse_capture(false);
                } else if wparam.0 == VK_F5.0 as usize {
                    config::reload();
                    sync_menu_checks();
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            WM_KILLFOCUS => {
                if resizing() {
                    finish_resize();
                }
                // The host game can focus its own window while the preview
                // remains the intended mouse-capture surface.
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            // Mouse sampling is gated by the foreground process in
            // consume_mouse_delta; focus changes only pause it. Esc and the
            // menu are the explicit ways to clear the capture preference.
            WM_ACTIVATE | WM_ACTIVATEAPP => DefWindowProcW(hwnd, message, wparam, lparam),
            WM_IME_SETCONTEXT => {
                DefWindowProcW(hwnd, message, wparam, LPARAM(suppress_ime_ui(lparam.0)))
            }
            WM_SETCURSOR => {
                if mouse_captured() {
                    let _ = SetCursor(None);
                    LRESULT(1)
                } else {
                    DefWindowProcW(hwnd, message, wparam, lparam)
                }
            }
            WM_CLOSE => {
                REQUEST_EXIT.store(true, Ordering::Release);
                let _ = ShowWindow(hwnd, windows::Win32::UI::WindowsAndMessaging::SW_HIDE);
                LRESULT(0)
            }
            WM_DESTROY => {
                if resizing() {
                    finish_resize();
                }
                REQUEST_EXIT.store(true, Ordering::Release);
                MOUSE_CAPTURED.store(false, Ordering::Release);
                WINDOW_HANDLE.store(ptr::null_mut(), Ordering::Release);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(left: bool, right: bool, top: bool, bottom: bool) -> ResizeEdge {
        ResizeEdge {
            left,
            right,
            top,
            bottom,
        }
    }

    #[test]
    fn resizing_from_the_right_changes_only_the_right_edge() {
        let start = RECT {
            left: 10,
            top: 20,
            right: 810,
            bottom: 620,
        };
        let resized = resized_window_rect(start, edge(false, true, false, false), 120, 50);
        assert_eq!(resized.left, 10);
        assert_eq!(resized.top, 20);
        assert_eq!(resized.right, 930);
        assert_eq!(resized.bottom, 620);
    }

    #[test]
    fn resizing_respects_the_minimum_window_size() {
        let start = RECT {
            left: 10,
            top: 20,
            right: 610,
            bottom: 420,
        };
        let resized = resized_window_rect(start, edge(true, false, true, false), 500, 500);
        assert_eq!(resized.left, start.right - MIN_WINDOW_WIDTH);
        assert_eq!(resized.top, start.bottom - MIN_WINDOW_HEIGHT);
        assert_eq!(resized.right, start.right);
        assert_eq!(resized.bottom, start.bottom);
    }

    #[test]
    fn ime_ui_is_suppressed_without_changing_other_context_flags() {
        let other_flags = 0x1234;
        let lparam = (ISC_SHOWUICOMPOSITIONWINDOW | other_flags) as isize;
        assert_eq!(suppress_ime_ui(lparam), other_flags as isize);
    }
}
