use super::*;

pub(super) unsafe fn paint_frame(dc: windows::Win32::Graphics::Gdi::HDC) {
    let mut client = RECT::default();
    let handle = hwnd();
    if handle.is_invalid() || unsafe { GetClientRect(handle, &mut client).is_err() } {
        return;
    }
    let width = (client.right - client.left).max(1);
    let height = (client.bottom - client.top).max(1);

    unsafe {
        let black = CreateSolidBrush(COLORREF(0));
        let _ = FillRect(dc, &client, black);
        let _ = windows::Win32::Graphics::Gdi::DeleteObject(black.into());
    }

    let frame = frame_slot().lock().ok().and_then(|guard| guard.clone());
    if let Some(frame) = frame {
        if frame.width == 0 || frame.height == 0 {
            return;
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: frame.width as i32,
                biHeight: -(frame.height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        unsafe {
            StretchDIBits(
                dc,
                0,
                0,
                width,
                height,
                0,
                0,
                frame.width as i32,
                frame.height as i32,
                Some(frame.bgra.as_ptr().cast()),
                &info,
                DIB_RGB_COLORS,
                SRCCOPY,
            );
        }
        return;
    }

    let mut text: Vec<u16> = "Waiting for the OpenXR application's first frame...\0"
        .encode_utf16()
        .collect();
    let mut rect = client;
    unsafe {
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, COLORREF(0x00e8e8e8));
        DrawTextW(
            dc,
            &mut text,
            &mut rect,
            windows::Win32::Graphics::Gdi::DT_CENTER
                | windows::Win32::Graphics::Gdi::DT_VCENTER
                | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
        );
    }
}
