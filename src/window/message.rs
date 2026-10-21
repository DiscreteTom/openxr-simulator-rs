use super::menu::handle_command;
use super::paint::paint_frame;
use super::*;

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
                set_mouse_capture(false);
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            WM_ACTIVATE | WM_ACTIVATEAPP => {
                let active = (wparam.0 & 0xffff) != 0;
                if !active {
                    set_mouse_capture(false);
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
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
                REQUEST_EXIT.store(true, Ordering::Release);
                MOUSE_CAPTURED.store(false, Ordering::Release);
                WINDOW_HANDLE.store(ptr::null_mut(), Ordering::Release);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
