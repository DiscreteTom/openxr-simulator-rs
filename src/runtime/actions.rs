use super::*;

use std::ffi::CStr;

use openxr_sys::Handle;

pub(super) unsafe extern "system" fn xrCreateActionSet(
    _instance: xr::Instance,
    create_info: *const xr::ActionSetCreateInfo,
    action_set: *mut xr::ActionSet,
) -> xr::Result {
    if create_info.is_null() || action_set.is_null() {
        return VALIDATION_FAILURE;
    }
    let name = fixed_string(&unsafe { (*create_info).action_set_name });
    let mut state = lock_runtime();
    let handle = xr::ActionSet::from_raw(state.allocate_handle());
    state.action_sets.insert(handle.into_raw(), name);
    unsafe { *action_set = handle };
    SUCCESS
}

pub(super) unsafe extern "system" fn xrDestroyActionSet(_action_set: xr::ActionSet) -> xr::Result {
    SUCCESS
}

pub(super) unsafe extern "system" fn xrCreateAction(
    _action_set: xr::ActionSet,
    create_info: *const xr::ActionCreateInfo,
    action: *mut xr::Action,
) -> xr::Result {
    if create_info.is_null() || action.is_null() {
        return VALIDATION_FAILURE;
    }
    let info = unsafe { &*create_info };
    let name = fixed_string(&info.action_name);
    let mut state = lock_runtime();
    let mut hands = [false, false];
    if info.count_subaction_paths > 0 && !info.subaction_paths.is_null() {
        for index in 0..info.count_subaction_paths {
            let path = unsafe { *info.subaction_paths.add(index as usize) };
            if let Some(text) = state.paths.get(&path.into_raw()) {
                if text.contains("/left") {
                    hands[0] = true;
                }
                if text.contains("/right") {
                    hands[1] = true;
                }
            }
        }
    }
    if !hands[0] && !hands[1] {
        hands = [true, true];
    }
    let handle = xr::Action::from_raw(state.allocate_handle());
    state
        .actions
        .insert(handle.into_raw(), ActionData { name, hands });
    unsafe { *action = handle };
    SUCCESS
}

pub(super) unsafe extern "system" fn xrDestroyAction(_action: xr::Action) -> xr::Result {
    SUCCESS
}

pub(super) unsafe extern "system" fn xrSuggestInteractionProfileBindings(
    _instance: xr::Instance,
    suggested: *const xr::InteractionProfileSuggestedBinding,
) -> xr::Result {
    if suggested.is_null() {
        return VALIDATION_FAILURE;
    }
    let mut state = lock_runtime();
    let profile = unsafe { (*suggested).interaction_profile };
    if profile != xr::Path::NULL && !state.suggested_profiles.contains(&profile) {
        state.suggested_profiles.push(profile);
    }
    SUCCESS
}

pub(super) unsafe extern "system" fn xrAttachSessionActionSets(
    session: xr::Session,
    attach_info: *const xr::SessionActionSetsAttachInfo,
) -> xr::Result {
    if attach_info.is_null() {
        return VALIDATION_FAILURE;
    }
    let mut state = lock_runtime();
    state.active_profile = state
        .suggested_profiles
        .iter()
        .copied()
        .find(|path| {
            state
                .paths
                .get(&path.into_raw())
                .is_some_and(|name| name.contains("oculus/touch"))
        })
        .or_else(|| state.suggested_profiles.first().copied())
        .unwrap_or(xr::Path::NULL);
    state
        .events
        .push_back(RuntimeEvent::InteractionProfileChanged { session });
    SUCCESS
}

pub(super) unsafe extern "system" fn xrGetActionStateBoolean(
    _session: xr::Session,
    get_info: *const xr::ActionStateGetInfo,
    value: *mut xr::ActionStateBoolean,
) -> xr::Result {
    if get_info.is_null() || value.is_null() {
        return VALIDATION_FAILURE;
    }
    let state = lock_runtime();
    let get_info = unsafe { &*get_info };
    let action = state.actions.get(&get_info.action.into_raw());
    let name = action
        .map(|action| action.name.to_ascii_lowercase())
        .unwrap_or_default();
    let hand = action_hand(&state, action, get_info.subaction_path, &name);
    let pressed = state.controllers[hand].boolean_state(&name);
    unsafe {
        *value = xr::ActionStateBoolean {
            ty: xr::StructureType::ACTION_STATE_BOOLEAN,
            next: ptr::null_mut(),
            current_state: pressed.into(),
            changed_since_last_sync: xr::FALSE,
            last_change_time: xr::Time::from_nanos(now_ns()),
            is_active: xr::TRUE,
        };
    }
    SUCCESS
}

pub(super) unsafe extern "system" fn xrGetActionStateFloat(
    _session: xr::Session,
    get_info: *const xr::ActionStateGetInfo,
    value: *mut xr::ActionStateFloat,
) -> xr::Result {
    if get_info.is_null() || value.is_null() {
        return VALIDATION_FAILURE;
    }
    let state = lock_runtime();
    let get_info = unsafe { &*get_info };
    let action = state.actions.get(&get_info.action.into_raw());
    let name = action
        .map(|action| action.name.to_ascii_lowercase())
        .unwrap_or_default();
    let hand = action_hand(&state, action, get_info.subaction_path, &name);
    let amount = state.controllers[hand].float_state(&name).unwrap_or(0.0);
    unsafe {
        *value = xr::ActionStateFloat {
            ty: xr::StructureType::ACTION_STATE_FLOAT,
            next: ptr::null_mut(),
            current_state: amount,
            changed_since_last_sync: xr::FALSE,
            last_change_time: xr::Time::from_nanos(now_ns()),
            is_active: xr::TRUE,
        };
    }
    SUCCESS
}

pub(super) unsafe extern "system" fn xrGetActionStatePose(
    _session: xr::Session,
    _get_info: *const xr::ActionStateGetInfo,
    value: *mut xr::ActionStatePose,
) -> xr::Result {
    if value.is_null() {
        return VALIDATION_FAILURE;
    }
    unsafe {
        *value = xr::ActionStatePose {
            ty: xr::StructureType::ACTION_STATE_POSE,
            next: ptr::null_mut(),
            is_active: xr::TRUE,
        };
    }
    SUCCESS
}

pub(super) unsafe extern "system" fn xrGetActionStateVector2f(
    _session: xr::Session,
    get_info: *const xr::ActionStateGetInfo,
    value: *mut xr::ActionStateVector2f,
) -> xr::Result {
    if get_info.is_null() || value.is_null() {
        return VALIDATION_FAILURE;
    }
    let state = lock_runtime();
    let get_info = unsafe { &*get_info };
    let action = state.actions.get(&get_info.action.into_raw());
    let name = action
        .map(|action| action.name.to_ascii_lowercase())
        .unwrap_or_default();
    let hand = action_hand(&state, action, get_info.subaction_path, &name);
    let current =
        if action_name_matches(&name, "thumbstick") || action_name_matches(&name, "joystick") {
            state.controllers[hand].thumbstick
        } else {
            xr::Vector2f { x: 0.0, y: 0.0 }
        };
    unsafe {
        *value = xr::ActionStateVector2f {
            ty: xr::StructureType::ACTION_STATE_VECTOR2F,
            next: ptr::null_mut(),
            current_state: current,
            changed_since_last_sync: xr::FALSE,
            last_change_time: xr::Time::from_nanos(now_ns()),
            is_active: xr::TRUE,
        };
    }
    SUCCESS
}

fn action_hand(
    state: &RuntimeState,
    action: Option<&ActionData>,
    subaction_path: xr::Path,
    name: &str,
) -> usize {
    if action_name_matches(name, "button_x") || action_name_matches(name, "button_y") {
        return 0;
    }
    if action_name_matches(name, "button_a") || action_name_matches(name, "button_b") {
        return 1;
    }
    if subaction_path != xr::Path::NULL
        && let Some(path) = state.paths.get(&subaction_path.into_raw())
    {
        if path.contains("/left") {
            return 0;
        }
        if path.contains("/right") {
            return 1;
        }
    }
    if let Some(action) = action {
        if action.hands[0] && !action.hands[1] {
            return 0;
        }
        if action.hands[1] && !action.hands[0] {
            return 1;
        }
    }
    1
}

pub(super) unsafe extern "system" fn xrSyncActions(
    _session: xr::Session,
    _sync_info: *const xr::ActionsSyncInfo,
) -> xr::Result {
    SUCCESS
}

pub(super) unsafe extern "system" fn xrStringToPath(
    _instance: xr::Instance,
    path_string: *const c_char,
    path: *mut xr::Path,
) -> xr::Result {
    if path_string.is_null() || path.is_null() {
        return VALIDATION_FAILURE;
    }
    let Ok(text) = unsafe { CStr::from_ptr(path_string) }.to_str() else {
        return VALIDATION_FAILURE;
    };
    let id = path_id(text);
    let mut state = lock_runtime();
    state.paths.insert(id, text.to_owned());
    state.path_reverse.insert(text.to_owned(), id);
    unsafe { *path = xr::Path::from_raw(id) };
    SUCCESS
}

pub(super) unsafe extern "system" fn xrPathToString(
    _instance: xr::Instance,
    path: xr::Path,
    capacity: u32,
    count: *mut u32,
    buffer: *mut c_char,
) -> xr::Result {
    let state = lock_runtime();
    let Some(text) = state.paths.get(&path.into_raw()) else {
        return PATH_INVALID;
    };
    let required = text.len() + 1;
    if !count.is_null() {
        unsafe { *count = required as u32 };
    }
    if capacity == 0 {
        return SUCCESS;
    }
    if (capacity as usize) < required || buffer.is_null() {
        return SIZE_INSUFFICIENT;
    }
    unsafe {
        ptr::copy_nonoverlapping(text.as_ptr(), buffer.cast::<u8>(), text.len());
        *buffer.add(text.len()) = 0;
    }
    SUCCESS
}

pub(super) unsafe extern "system" fn xrGetCurrentInteractionProfile(
    _session: xr::Session,
    _top_level_user_path: xr::Path,
    profile: *mut xr::InteractionProfileState,
) -> xr::Result {
    if profile.is_null() {
        return VALIDATION_FAILURE;
    }
    let state = lock_runtime();
    unsafe {
        *profile = xr::InteractionProfileState {
            ty: xr::StructureType::INTERACTION_PROFILE_STATE,
            next: ptr::null_mut(),
            interaction_profile: state.active_profile,
        };
    }
    SUCCESS
}

pub(super) unsafe extern "system" fn xrEnumerateBoundSourcesForAction(
    _session: xr::Session,
    _enumerate_info: *const xr::BoundSourcesForActionEnumerateInfo,
    _capacity: u32,
    count: *mut u32,
    _sources: *mut xr::Path,
) -> xr::Result {
    if !count.is_null() {
        unsafe { *count = 0 };
    }
    SUCCESS
}

pub(super) unsafe extern "system" fn xrGetInputSourceLocalizedName(
    _session: xr::Session,
    _get_info: *const xr::InputSourceLocalizedNameGetInfo,
    capacity: u32,
    count: *mut u32,
    buffer: *mut c_char,
) -> xr::Result {
    let text = "Simulated controller";
    let required = text.len() + 1;
    if !count.is_null() {
        unsafe { *count = required as u32 };
    }
    if capacity == 0 {
        return SUCCESS;
    }
    if (capacity as usize) < required || buffer.is_null() {
        return SIZE_INSUFFICIENT;
    }
    unsafe {
        ptr::copy_nonoverlapping(text.as_ptr(), buffer.cast::<u8>(), text.len());
        *buffer.add(text.len()) = 0;
    }
    SUCCESS
}

pub(super) unsafe extern "system" fn xrApplyHapticFeedback(
    _session: xr::Session,
    _haptic: *const xr::HapticActionInfo,
    _feedback: *const xr::HapticBaseHeader,
) -> xr::Result {
    SUCCESS
}

pub(super) unsafe extern "system" fn xrStopHapticFeedback(
    _session: xr::Session,
    _haptic: *const xr::HapticActionInfo,
) -> xr::Result {
    SUCCESS
}

pub(super) unsafe extern "system" fn xrGetReferenceSpaceBoundsRect(
    _session: xr::Session,
    _space_type: xr::ReferenceSpaceType,
    _bounds: *mut xr::Extent2Df,
) -> xr::Result {
    xr::Result::SPACE_BOUNDS_UNAVAILABLE
}
