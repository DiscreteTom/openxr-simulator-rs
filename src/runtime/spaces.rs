use super::*;

use openxr_sys::Handle;

pub(super) unsafe extern "system" fn xrLocateViews(
    _session: xr::Session,
    _locate_info: *const xr::ViewLocateInfo,
    view_state: *mut xr::ViewState,
    capacity: u32,
    count: *mut u32,
    views: *mut xr::View,
) -> xr::Result {
    if !count.is_null() {
        unsafe { *count = 2 };
    }
    if !view_state.is_null() {
        unsafe {
            (*view_state).ty = xr::StructureType::VIEW_STATE;
            (*view_state).view_state_flags = xr::ViewStateFlags::ORIENTATION_VALID
                | xr::ViewStateFlags::POSITION_VALID
                | xr::ViewStateFlags::ORIENTATION_TRACKED
                | xr::ViewStateFlags::POSITION_TRACKED;
        }
    }
    if capacity == 0 || views.is_null() {
        return SUCCESS;
    }
    let settings = config::snapshot();
    let state = lock_runtime();
    let fov = math::symmetric_fov(
        settings.fov.horizontal_degrees,
        settings.fov.vertical_degrees,
    );
    for eye in 0..capacity.min(2) as usize {
        unsafe {
            *views.add(eye) = xr::View {
                ty: xr::StructureType::VIEW,
                next: ptr::null_mut(),
                pose: math::eye_pose(
                    eye,
                    state.head.yaw,
                    state.head.pitch,
                    state.head.position,
                    settings.fov.ipd_meters,
                ),
                fov,
            };
        }
    }
    SUCCESS
}

pub(super) unsafe extern "system" fn xrEnumerateReferenceSpaces(
    _session: xr::Session,
    capacity: u32,
    count: *mut u32,
    spaces: *mut xr::ReferenceSpaceType,
) -> xr::Result {
    unsafe {
        write_count_buffer(
            capacity,
            count,
            &[
                xr::ReferenceSpaceType::VIEW,
                xr::ReferenceSpaceType::LOCAL,
                xr::ReferenceSpaceType::STAGE,
            ],
            spaces,
        )
    }
}

pub(super) unsafe extern "system" fn xrCreateReferenceSpace(
    _session: xr::Session,
    create_info: *const xr::ReferenceSpaceCreateInfo,
    space: *mut xr::Space,
) -> xr::Result {
    if create_info.is_null() || space.is_null() {
        return VALIDATION_FAILURE;
    }
    let kind = match unsafe { (*create_info).reference_space_type } {
        xr::ReferenceSpaceType::VIEW => SpaceKind::View,
        xr::ReferenceSpaceType::STAGE => SpaceKind::Stage,
        xr::ReferenceSpaceType::LOCAL | xr::ReferenceSpaceType::LOCAL_FLOOR => SpaceKind::Local,
        _ => return xr::Result::ERROR_REFERENCE_SPACE_UNSUPPORTED,
    };
    let mut state = lock_runtime();
    let handle = xr::Space::from_raw(state.allocate_handle());
    state.spaces.insert(handle.into_raw(), SpaceData { kind });
    unsafe { *space = handle };
    SUCCESS
}

pub(super) unsafe extern "system" fn xrDestroySpace(space: xr::Space) -> xr::Result {
    let mut state = lock_runtime();
    if state.spaces.remove(&space.into_raw()).is_some() {
        SUCCESS
    } else {
        HANDLE_INVALID
    }
}

pub(super) unsafe extern "system" fn xrCreateActionSpace(
    _session: xr::Session,
    create_info: *const xr::ActionSpaceCreateInfo,
    space: *mut xr::Space,
) -> xr::Result {
    if create_info.is_null() || space.is_null() {
        return VALIDATION_FAILURE;
    }
    let info = unsafe { &*create_info };
    let mut state = lock_runtime();
    let path = state
        .paths
        .get(&info.subaction_path.into_raw())
        .cloned()
        .unwrap_or_default();
    let hand = if path.contains("/left") { 0 } else { 1 };
    let handle = xr::Space::from_raw(state.allocate_handle());
    state.spaces.insert(
        handle.into_raw(),
        SpaceData {
            kind: SpaceKind::Controller {
                hand,
                pose: info.pose_in_action_space,
            },
        },
    );
    unsafe { *space = handle };
    SUCCESS
}

pub(super) unsafe extern "system" fn xrLocateSpace(
    space: xr::Space,
    _base_space: xr::Space,
    _time: xr::Time,
    location: *mut xr::SpaceLocation,
) -> xr::Result {
    if location.is_null() {
        return VALIDATION_FAILURE;
    }
    let state = lock_runtime();
    let Some(data) = state.spaces.get(&space.into_raw()) else {
        return HANDLE_INVALID;
    };
    let flags = xr::SpaceLocationFlags::POSITION_VALID
        | xr::SpaceLocationFlags::ORIENTATION_VALID
        | xr::SpaceLocationFlags::POSITION_TRACKED
        | xr::SpaceLocationFlags::ORIENTATION_TRACKED;
    let pose = match data.kind {
        SpaceKind::View => state.head_pose(),
        SpaceKind::Local | SpaceKind::Stage => xr::Posef::IDENTITY,
        SpaceKind::Controller { hand, pose } => {
            let lateral = if hand == 0 { -0.18 } else { 0.18 };
            let offset = math::rotate_xz(
                state.head.yaw,
                Vec3::new(
                    lateral + pose.position.x,
                    pose.position.y - 0.28,
                    pose.position.z,
                ),
            );
            let mut result = state.head_pose();
            result.position.x += offset.x;
            result.position.y += offset.y;
            result.position.z += offset.z;
            result
        }
    };
    unsafe {
        *location = xr::SpaceLocation {
            ty: xr::StructureType::SPACE_LOCATION,
            next: ptr::null_mut(),
            location_flags: flags,
            pose,
        };
    }
    SUCCESS
}
