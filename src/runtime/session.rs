use super::*;

use std::ffi::c_void;

use windows::Win32::Graphics::Direct3D11::ID3D11Device;

use windows::core::Interface;

pub(super) unsafe extern "system" fn xrCreateSession(
    _instance: xr::Instance,
    create_info: *const xr::SessionCreateInfo,
    session: *mut xr::Session,
) -> xr::Result {
    if create_info.is_null() || session.is_null() {
        return VALIDATION_FAILURE;
    }
    let mut binding = unsafe { (*create_info).next } as *const xr::BaseInStructure;
    let mut d3d11_binding = None;
    while !binding.is_null() {
        let entry = unsafe { &*binding };
        if entry.ty == xr::StructureType::GRAPHICS_BINDING_D3D11_KHR {
            d3d11_binding = Some(unsafe { &*(binding.cast::<xr::GraphicsBindingD3D11KHR>()) });
            break;
        }
        binding = entry.next;
    }
    let Some(binding) = d3d11_binding else {
        crate::logger::error(format_args!(
            "only the XR_KHR_D3D11_enable graphics binding is implemented"
        ));
        return GRAPHICS_DEVICE_INVALID;
    };
    if binding.device.is_null() {
        return GRAPHICS_DEVICE_INVALID;
    }
    let borrowed: ID3D11Device = unsafe { ID3D11Device::from_raw(binding.device.cast()) };
    let device = borrowed.clone();
    std::mem::forget(borrowed);
    let mirror = match d3d11::Mirror::new(device) {
        Ok(mirror) => mirror,
        Err(error) => {
            crate::logger::error(format_args!("D3D11 mirror setup failed: {error}"));
            return GRAPHICS_DEVICE_INVALID;
        }
    };

    let mut state = lock_runtime();
    if state.instance.is_none() {
        return HANDLE_INVALID;
    }
    state.swapchains.clear();
    let handle = xr::Session::from_raw(state.allocate_handle());
    state.session = Some(SessionData {
        handle,
        state: xr::SessionState::IDLE,
        mirror,
        preview: None,
        preview_attempted: false,
    });
    state.push_session_state(xr::SessionState::READY);
    state.reset_pose();
    drop(state);

    if let Err(error) = window::create() {
        crate::logger::error(format_args!("preview window creation failed: {error}"));
    }
    unsafe { *session = handle };
    crate::logger::info(format_args!(
        "created D3D11 session 0x{:x}",
        handle.into_raw()
    ));
    SUCCESS
}

pub(super) unsafe extern "system" fn xrDestroySession(session: xr::Session) -> xr::Result {
    let mut state = lock_runtime();
    if state.session.as_ref().map(|s| s.handle) != Some(session) {
        return HANDLE_INVALID;
    }
    state.session = None;
    state.swapchains.clear();
    state.spaces.clear();
    SUCCESS
}

pub(super) unsafe extern "system" fn xrEnumerateSwapchainFormats(
    _session: xr::Session,
    capacity: u32,
    count: *mut u32,
    formats: *mut i64,
) -> xr::Result {
    let values: Vec<i64> = d3d11::COLOR_FORMATS
        .iter()
        .chain(d3d11::DEPTH_FORMATS.iter())
        .map(|format| format.0 as i64)
        .collect();
    unsafe { write_count_buffer(capacity, count, &values, formats) }
}

pub(super) unsafe extern "system" fn xrCreateSwapchain(
    session: xr::Session,
    create_info: *const xr::SwapchainCreateInfo,
    swapchain: *mut xr::Swapchain,
) -> xr::Result {
    if create_info.is_null() || swapchain.is_null() {
        return VALIDATION_FAILURE;
    }
    let info = unsafe { &*create_info };
    if info.sample_count != 1 || info.face_count != 1 || !d3d11::is_supported_format(info.format) {
        return xr::Result::ERROR_SWAPCHAIN_FORMAT_UNSUPPORTED;
    }
    let mut state = lock_runtime();
    let Some(session_data) = state.session.as_ref() else {
        return HANDLE_INVALID;
    };
    if session_data.handle != session {
        return HANDLE_INVALID;
    }
    let format = windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT(info.format as i32);
    let depth = d3d11::is_depth_format(info.format);
    // OpenXR lets the runtime choose its internal image count. Three images
    // each with the requested array size give an application enough room for
    // typical double/triple buffering while keeping one left/right slice pair.
    let mut images = Vec::with_capacity(3);
    for _ in 0..3 {
        match d3d11::create_texture(
            session_data.mirror.device(),
            &d3d11::TextureSpec {
                format,
                width: info.width,
                height: info.height,
                array_size: info.array_size.max(1),
                mip_levels: info.mip_count.max(1),
                mutable_format: false,
                depth,
            },
        ) {
            Ok(texture) => images.push(texture),
            Err(error) => {
                crate::logger::error(format_args!(
                    "CreateTexture2D failed for swapchain: {error}"
                ));
                return RUNTIME_FAILURE;
            }
        }
    }
    let handle_raw = state.allocate_handle();
    let handle = xr::Swapchain::from_raw(handle_raw);
    state.swapchains.insert(
        handle_raw,
        SwapchainData {
            images,
            next_image: 0,
            acquired: None,
            last_released: None,
            is_depth: depth,
        },
    );
    unsafe { *swapchain = handle };
    SUCCESS
}

pub(super) unsafe extern "system" fn xrDestroySwapchain(swapchain: xr::Swapchain) -> xr::Result {
    let mut state = lock_runtime();
    if state.swapchains.remove(&swapchain.into_raw()).is_none() {
        HANDLE_INVALID
    } else {
        SUCCESS
    }
}

pub(super) unsafe extern "system" fn xrEnumerateSwapchainImages(
    swapchain: xr::Swapchain,
    capacity: u32,
    count: *mut u32,
    images: *mut xr::SwapchainImageBaseHeader,
) -> xr::Result {
    let state = lock_runtime();
    let Some(chain) = state.swapchains.get(&swapchain.into_raw()) else {
        return HANDLE_INVALID;
    };
    if !count.is_null() {
        unsafe { *count = chain.images.len() as u32 };
    }
    if capacity == 0 || images.is_null() {
        return SUCCESS;
    }
    let textures: Vec<*mut c_void> = chain
        .images
        .iter()
        .map(|texture| d3d11::texture_raw(texture).cast())
        .collect();
    unsafe {
        write_d3d11_image_headers(images, capacity.min(chain.images.len() as u32), &textures);
    }
    SUCCESS
}

unsafe fn write_d3d11_image_headers(
    images: *mut xr::SwapchainImageBaseHeader,
    capacity: u32,
    textures: &[*mut c_void],
) {
    // The parameter itself is a base-header pointer, but the caller's array
    // stride is sizeof(XrSwapchainImageD3D11KHR). Advancing the base pointer
    // would write image 1 into the middle of image 0.
    let d3d_images = images.cast::<xr::SwapchainImageD3D11KHR>();
    for (index, texture) in textures.iter().take(capacity as usize).enumerate() {
        let d3d_image = unsafe { &mut *d3d_images.add(index) };
        d3d_image.ty = xr::StructureType::SWAPCHAIN_IMAGE_D3D11_KHR;
        d3d_image.next = ptr::null_mut();
        d3d_image.texture = (*texture).cast();
    }
}

pub(super) unsafe extern "system" fn xrAcquireSwapchainImage(
    swapchain: xr::Swapchain,
    _acquire_info: *const xr::SwapchainImageAcquireInfo,
    index: *mut u32,
) -> xr::Result {
    if index.is_null() {
        return VALIDATION_FAILURE;
    }
    let mut state = lock_runtime();
    let Some(chain) = state.swapchains.get_mut(&swapchain.into_raw()) else {
        return HANDLE_INVALID;
    };
    let image = chain.next_image % chain.images.len().max(1);
    chain.next_image = (image + 1) % chain.images.len().max(1);
    chain.acquired = Some(image as u32);
    unsafe { *index = image as u32 };
    SUCCESS
}

pub(super) unsafe extern "system" fn xrWaitSwapchainImage(
    swapchain: xr::Swapchain,
    _wait_info: *const xr::SwapchainImageWaitInfo,
) -> xr::Result {
    let state = lock_runtime();
    if state.swapchains.contains_key(&swapchain.into_raw()) {
        SUCCESS
    } else {
        HANDLE_INVALID
    }
}

pub(super) unsafe extern "system" fn xrReleaseSwapchainImage(
    swapchain: xr::Swapchain,
    _release_info: *const xr::SwapchainImageReleaseInfo,
) -> xr::Result {
    let mut state = lock_runtime();
    let Some(chain) = state.swapchains.get_mut(&swapchain.into_raw()) else {
        return HANDLE_INVALID;
    };
    chain.last_released = chain.acquired.take();
    SUCCESS
}

pub(super) unsafe extern "system" fn xrBeginSession(
    session: xr::Session,
    _begin_info: *const xr::SessionBeginInfo,
) -> xr::Result {
    let mut state = lock_runtime();
    let Some(current) = state.session.as_ref() else {
        return HANDLE_INVALID;
    };
    if current.handle != session {
        return HANDLE_INVALID;
    }
    state.push_session_state(xr::SessionState::SYNCHRONIZED);
    state.push_session_state(xr::SessionState::VISIBLE);
    state.push_session_state(xr::SessionState::FOCUSED);
    state.next_frame = Instant::now();
    SUCCESS
}

pub(super) unsafe extern "system" fn xrEndSession(session: xr::Session) -> xr::Result {
    let mut state = lock_runtime();
    let Some(current) = state.session.as_ref() else {
        return HANDLE_INVALID;
    };
    if current.handle != session {
        return HANDLE_INVALID;
    }
    state.push_session_state(xr::SessionState::STOPPING);
    state.push_session_state(xr::SessionState::IDLE);
    SUCCESS
}

pub(super) unsafe extern "system" fn xrRequestExitSession(session: xr::Session) -> xr::Result {
    let mut state = lock_runtime();
    let Some(current) = state.session.as_ref() else {
        return HANDLE_INVALID;
    };
    if current.handle != session {
        return HANDLE_INVALID;
    }
    state.push_session_state(xr::SessionState::EXITING);
    SUCCESS
}

pub(super) unsafe extern "system" fn xrWaitFrame(
    _session: xr::Session,
    _wait_info: *const xr::FrameWaitInfo,
    frame_state: *mut xr::FrameState,
) -> xr::Result {
    if frame_state.is_null() {
        return VALIDATION_FAILURE;
    }
    window::pump_messages();
    let now = Instant::now();
    let (deadline, should_sleep) = {
        let state = lock_runtime();
        (state.next_frame, state.next_frame > now)
    };
    if should_sleep {
        let duration = deadline.saturating_duration_since(now);
        if duration > Duration::from_millis(1) {
            std::thread::sleep(duration.min(Duration::from_millis(20)));
        }
    }

    let mut state = lock_runtime();
    state.update_input();
    state.next_frame = Instant::now() + Duration::from_nanos(1_000_000_000 / 90);
    let should_render = state.session.as_ref().is_some_and(|session| {
        !matches!(
            session.state,
            xr::SessionState::IDLE
                | xr::SessionState::STOPPING
                | xr::SessionState::LOSS_PENDING
                | xr::SessionState::EXITING
        )
    });
    let display_time = now_ns();
    unsafe {
        *frame_state = xr::FrameState {
            ty: xr::StructureType::FRAME_STATE,
            next: ptr::null_mut(),
            predicted_display_time: xr::Time::from_nanos(display_time),
            predicted_display_period: xr::Duration::from_nanos(1_000_000_000 / 90),
            should_render: xr::Bool32::from(should_render),
        };
    }
    SUCCESS
}

pub(super) unsafe extern "system" fn xrBeginFrame(
    _session: xr::Session,
    _begin_info: *const xr::FrameBeginInfo,
) -> xr::Result {
    SUCCESS
}

pub(super) unsafe extern "system" fn xrPollEvent(
    _instance: xr::Instance,
    event: *mut xr::EventDataBuffer,
) -> xr::Result {
    if event.is_null() {
        return VALIDATION_FAILURE;
    }
    let mut state = lock_runtime();
    let Some(event_data) = state.events.pop_front() else {
        return EVENT_UNAVAILABLE;
    };
    unsafe { ptr::write_bytes(event, 0, 1) };
    match event_data {
        RuntimeEvent::SessionStateChanged { session, state } => {
            let data = xr::EventDataSessionStateChanged {
                ty: xr::StructureType::EVENT_DATA_SESSION_STATE_CHANGED,
                next: ptr::null_mut(),
                session,
                state,
                time: xr::Time::from_nanos(now_ns()),
            };
            unsafe { ptr::write(event.cast(), data) };
        }
        RuntimeEvent::InteractionProfileChanged { session } => {
            let data = xr::EventDataInteractionProfileChanged {
                ty: xr::StructureType::EVENT_DATA_INTERACTION_PROFILE_CHANGED,
                next: ptr::null_mut(),
                session,
            };
            unsafe { ptr::write(event.cast(), data) };
        }
    }
    SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d3d11_swapchain_image_array_uses_typed_stride() {
        let mut images: [xr::SwapchainImageD3D11KHR; 3] = unsafe { std::mem::zeroed() };
        let first = 0x1000usize as *mut c_void;
        let second = 0x2000usize as *mut c_void;

        unsafe {
            write_d3d11_image_headers(
                images.as_mut_ptr().cast(),
                images.len() as u32,
                &[first, second],
            );
        }

        assert_eq!(images[0].ty, xr::StructureType::SWAPCHAIN_IMAGE_D3D11_KHR);
        assert_eq!(images[1].ty, xr::StructureType::SWAPCHAIN_IMAGE_D3D11_KHR);
        assert_eq!(images[0].texture, first.cast());
        assert_eq!(images[1].texture, second.cast());
    }
}
