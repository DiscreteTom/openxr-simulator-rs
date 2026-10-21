use super::*;

use openxr_sys::Handle;

pub(super) unsafe extern "system" fn xrLocateViews(
    _session: xr::Session,
    locate_info: *const xr::ViewLocateInfo,
    view_state: *mut xr::ViewState,
    capacity: u32,
    count: *mut u32,
    views: *mut xr::View,
) -> xr::Result {
    if locate_info.is_null() {
        return VALIDATION_FAILURE;
    }
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
    let requested_space = unsafe { (*locate_info).space };
    let Some(base) = state.spaces.get(&requested_space.into_raw()) else {
        return HANDLE_INVALID;
    };
    let base_world = space_world_pose(&state, base.kind);
    let fov = math::symmetric_fov(
        settings.fov.horizontal_degrees,
        settings.fov.vertical_degrees,
    );
    let head_world = state.head_pose();
    for eye in 0..capacity.min(2) as usize {
        let eye_world =
            math::pose_compose(head_world, math::eye_offset(eye, settings.fov.ipd_meters));
        unsafe {
            *views.add(eye) = xr::View {
                ty: xr::StructureType::VIEW,
                next: ptr::null_mut(),
                pose: math::pose_relative(eye_world, base_world),
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
    base_space: xr::Space,
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
    let Some(base) = state.spaces.get(&base_space.into_raw()) else {
        return HANDLE_INVALID;
    };
    let flags = xr::SpaceLocationFlags::POSITION_VALID
        | xr::SpaceLocationFlags::ORIENTATION_VALID
        | xr::SpaceLocationFlags::POSITION_TRACKED
        | xr::SpaceLocationFlags::ORIENTATION_TRACKED;
    let pose = math::pose_relative(
        space_world_pose(&state, data.kind),
        space_world_pose(&state, base.kind),
    );
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

fn space_world_pose(state: &RuntimeState, kind: SpaceKind) -> xr::Posef {
    match kind {
        SpaceKind::View => state.head_pose(),
        SpaceKind::Stage => xr::Posef::IDENTITY,
        SpaceKind::Local => xr::Posef {
            orientation: xr::Quaternionf::IDENTITY,
            position: xr::Vector3f {
                x: 0.0,
                y: SIMULATED_EYE_HEIGHT,
                z: 0.0,
            },
        },
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_space_contains_only_eye_offsets() {
        let mut state = RuntimeState::new();
        state.head.position = Vec3::new(0.4, 1.7, -0.2);
        state.head.yaw = 0.7;
        state.head.pitch = -0.15;
        let head = space_world_pose(&state, SpaceKind::View);
        let eye = math::pose_compose(head, math::eye_offset(1, 0.064));
        let relative = math::pose_relative(eye, head);
        assert!((relative.position.x - 0.032).abs() < 1e-5);
        assert!(relative.position.y.abs() < 1e-5);
        assert!(relative.position.z.abs() < 1e-5);
        assert!((relative.orientation.w - 1.0).abs() < 1e-5);
    }

    #[test]
    fn local_space_origin_is_at_initial_head_height() {
        let state = RuntimeState::new();
        let stage = space_world_pose(&state, SpaceKind::Stage);
        let local = space_world_pose(&state, SpaceKind::Local);
        let relative = math::pose_relative(local, stage);
        assert!((relative.position.y - SIMULATED_EYE_HEIGHT).abs() < 1e-5);
    }

    #[test]
    fn view_located_from_local_starts_at_the_origin() {
        let state = RuntimeState::new();
        let view = space_world_pose(&state, SpaceKind::View);
        let local = space_world_pose(&state, SpaceKind::Local);
        let relative = math::pose_relative(view, local);
        assert!(relative.position.x.abs() < 1e-5);
        assert!(relative.position.y.abs() < 1e-5);
        assert!(relative.position.z.abs() < 1e-5);
    }
}
