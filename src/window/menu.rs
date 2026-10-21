use super::*;

pub(super) fn build_menu() -> Result<windows::Win32::UI::WindowsAndMessaging::HMENU, String> {
    unsafe {
        let menu = CreateMenu().map_err(|e| e.to_string())?;
        let view = CreateMenu().map_err(|e| e.to_string())?;
        let fov = CreateMenu().map_err(|e| e.to_string())?;
        let controls = CreateMenu().map_err(|e| e.to_string())?;
        let help = CreateMenu().map_err(|e| e.to_string())?;

        append(
            &view,
            MF_STRING,
            ID_VIEW_STEREO as usize,
            w!("Stereo side-by-side"),
        )?;
        append(&view, MF_STRING, ID_VIEW_LEFT as usize, w!("Left eye only"))?;
        append(
            &view,
            MF_STRING,
            ID_VIEW_RIGHT as usize,
            w!("Right eye only"),
        )?;

        append(
            &fov,
            MF_STRING,
            ID_FOV_DECREASE as usize,
            w!("Decrease horizontal FOV  [ - ]"),
        )?;
        append(
            &fov,
            MF_STRING,
            ID_FOV_INCREASE as usize,
            w!("Increase horizontal FOV  [ + ]"),
        )?;
        append(
            &fov,
            MF_STRING,
            ID_VFOV_DECREASE as usize,
            w!("Decrease vertical FOV"),
        )?;
        append(
            &fov,
            MF_STRING,
            ID_VFOV_INCREASE as usize,
            w!("Increase vertical FOV"),
        )?;
        append(
            &fov,
            MF_STRING,
            ID_IPD_DECREASE as usize,
            w!("Decrease IPD"),
        )?;
        append(
            &fov,
            MF_STRING,
            ID_IPD_INCREASE as usize,
            w!("Increase IPD"),
        )?;
        append(&fov, MF_SEPARATOR, 0, PCWSTR::null())?;
        append(
            &fov,
            MF_STRING,
            ID_FOV_RESET as usize,
            w!("Reset FOV and IPD"),
        )?;

        append(
            &controls,
            MF_STRING,
            ID_CAPTURE_MOUSE as usize,
            w!("Capture / release mouse"),
        )?;
        append(
            &controls,
            MF_STRING,
            ID_RESET_POSE as usize,
            w!("Reset pose"),
        )?;
        append(
            &controls,
            MF_STRING,
            ID_RESET_VIEW as usize,
            w!("Reset view and pose"),
        )?;
        append(&controls, MF_SEPARATOR, 0, PCWSTR::null())?;
        append(
            &controls,
            MF_STRING,
            ID_RELOAD_CONFIG as usize,
            w!("Reload settings.json  [ F5 ]"),
        )?;
        append(
            &controls,
            MF_STRING,
            ID_OPEN_CONFIG_FILE as usize,
            w!("Open settings.json"),
        )?;

        append(
            &help,
            MF_STRING,
            9001,
            w!("WASD move, Q/E down/up, mouse look"),
        )?;
        append(
            &help,
            MF_STRING | MF_GRAYED,
            9002,
            w!("F5 reload, F6 cycle eye, R reset pose"),
        )?;

        append(&menu, MF_POPUP, view.0 as usize, w!("View"))?;
        append(&menu, MF_POPUP, fov.0 as usize, w!("FOV / IPD"))?;
        append(&menu, MF_POPUP, controls.0 as usize, w!("Controls"))?;
        append(&menu, MF_POPUP, help.0 as usize, w!("Help"))?;
        Ok(menu)
    }
}

pub(super) unsafe fn append(
    menu: &windows::Win32::UI::WindowsAndMessaging::HMENU,
    flags: windows::Win32::UI::WindowsAndMessaging::MENU_ITEM_FLAGS,
    id: usize,
    text: PCWSTR,
) -> Result<(), String> {
    unsafe { AppendMenuW(*menu, flags, id, text) }.map_err(|e| e.to_string())
}

pub(super) fn sync_menu_checks() {
    let raw = MENU_HANDLE.load(Ordering::Acquire);
    if raw.is_null() {
        return;
    }
    let menu = windows::Win32::UI::WindowsAndMessaging::HMENU(raw);
    let settings = config::snapshot();
    let selected = match settings.display.view_mode {
        ViewMode::Stereo => ID_VIEW_STEREO,
        ViewMode::LeftEye => ID_VIEW_LEFT,
        ViewMode::RightEye => ID_VIEW_RIGHT,
    };
    unsafe {
        let _ = CheckMenuRadioItem(menu, ID_VIEW_STEREO, ID_VIEW_RIGHT, selected, 0);
    }
}

pub(super) fn handle_command(id: u32) {
    match id {
        ID_VIEW_STEREO => change_view(ViewMode::Stereo),
        ID_VIEW_LEFT => change_view(ViewMode::LeftEye),
        ID_VIEW_RIGHT => change_view(ViewMode::RightEye),
        ID_FOV_DECREASE => change_fov(-5.0, 0.0, 0.0),
        ID_FOV_INCREASE => change_fov(5.0, 0.0, 0.0),
        ID_VFOV_DECREASE => change_fov(0.0, -5.0, 0.0),
        ID_VFOV_INCREASE => change_fov(0.0, 5.0, 0.0),
        ID_IPD_DECREASE => change_fov(0.0, 0.0, -0.001),
        ID_IPD_INCREASE => change_fov(0.0, 0.0, 0.001),
        ID_FOV_RESET => {
            config::update(|settings| {
                settings.fov = Default::default();
            });
            config::persist();
        }
        ID_CAPTURE_MOUSE => set_mouse_capture(!mouse_captured()),
        ID_RESET_POSE | ID_RESET_VIEW => {
            RESET_POSE_REQUESTED.store(true, Ordering::Release);
        }
        ID_RELOAD_CONFIG => {
            config::reload();
            sync_menu_checks();
        }
        ID_OPEN_CONFIG_FILE => {
            config::persist();
            let _ = std::process::Command::new("explorer.exe")
                .arg(config::config_path())
                .spawn();
        }
        _ => {}
    }
}

fn change_view(mode: ViewMode) {
    config::update(|settings| settings.display.view_mode = mode);
    config::persist();
    sync_menu_checks();
}

fn change_fov(horizontal_delta: f32, vertical_delta: f32, ipd_delta: f32) {
    config::update(|settings| {
        settings.fov.horizontal_degrees += horizontal_delta;
        settings.fov.vertical_degrees += vertical_delta;
        settings.fov.ipd_meters += ipd_delta;
    });
    config::persist();
}
