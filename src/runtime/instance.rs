use super::*;

use std::ffi::CStr;
use std::time::Instant;

use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};

use super::dispatch::xrGetInstanceProcAddr;

/// Negotiates the loader/runtime interface required by the OpenXR loader.
///
/// # Safety
///
/// Both pointers must point to valid OpenXR negotiation structures owned by
/// the loader for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn xrNegotiateLoaderRuntimeInterface(
    loader_info: *const xr::NegotiateLoaderInfo,
    runtime_request: *mut xr::NegotiateRuntimeRequest,
) -> xr::Result {
    if loader_info.is_null() || runtime_request.is_null() {
        return VALIDATION_FAILURE;
    }
    config::init();
    START.get_or_init(Instant::now);
    let info = unsafe { &*loader_info };
    crate::logger::info(format_args!(
        "loader negotiation: interface {}..{}, API {}..{}",
        info.min_interface_version,
        info.max_interface_version,
        info.min_api_version,
        info.max_api_version
    ));
    if info.max_interface_version < xr::CURRENT_LOADER_RUNTIME_VERSION as u32 {
        return xr::Result::ERROR_INITIALIZATION_FAILED;
    }
    let request = unsafe { &mut *runtime_request };
    request.runtime_interface_version = xr::CURRENT_LOADER_RUNTIME_VERSION as u32;
    request.runtime_api_version = xr::Version::new(1, 0, 0);
    request.get_instance_proc_addr = Some(xrGetInstanceProcAddr);
    SUCCESS
}

pub(super) unsafe extern "system" fn xrEnumerateApiLayerProperties(
    _capacity: u32,
    count: *mut u32,
    _properties: *mut xr::ApiLayerProperties,
) -> xr::Result {
    if !count.is_null() {
        unsafe { *count = 0 };
    }
    SUCCESS
}

pub(super) unsafe extern "system" fn xrEnumerateInstanceExtensionProperties(
    layer_name: *const c_char,
    capacity: u32,
    count: *mut u32,
    properties: *mut xr::ExtensionProperties,
) -> xr::Result {
    if !layer_name.is_null() {
        let name = unsafe { CStr::from_ptr(layer_name) };
        if !name.is_empty() {
            return xr::Result::ERROR_LAYER_INVALID;
        }
    }
    if !count.is_null() {
        unsafe { *count = EXTENSIONS.len() as u32 };
    }
    if capacity == 0 || properties.is_null() {
        return SUCCESS;
    }
    for index in 0..capacity.min(EXTENSIONS.len() as u32) {
        let property = unsafe { &mut *properties.add(index as usize) };
        *property = xr::ExtensionProperties {
            ty: xr::StructureType::EXTENSION_PROPERTIES,
            next: ptr::null_mut(),
            extension_name: [0; xr::MAX_EXTENSION_NAME_SIZE],
            extension_version: 1,
        };
        unsafe {
            copy_chars(&mut property.extension_name, EXTENSIONS[index as usize]);
        }
    }
    SUCCESS
}

pub(super) unsafe extern "system" fn xrCreateInstance(
    create_info: *const xr::InstanceCreateInfo,
    instance: *mut xr::Instance,
) -> xr::Result {
    if create_info.is_null() || instance.is_null() {
        return VALIDATION_FAILURE;
    }
    config::init();
    let info = unsafe { &*create_info };
    let app_name = fixed_string(&info.application_info.application_name);
    if info.enabled_extension_count > 0 && info.enabled_extension_names.is_null() {
        return VALIDATION_FAILURE;
    }
    for index in 0..info.enabled_extension_count {
        let name = unsafe { *info.enabled_extension_names.add(index as usize) };
        let Ok(name) = unsafe { CStr::from_ptr(name) }.to_str() else {
            return VALIDATION_FAILURE;
        };
        if !EXTENSIONS.contains(&name) {
            return EXTENSION_NOT_PRESENT;
        }
    }
    let mut state = lock_runtime();
    if state.instance.is_some() {
        return xr::Result::ERROR_INITIALIZATION_FAILED;
    }
    let handle = xr::Instance::from_raw(state.allocate_handle());
    state.instance = Some(handle);
    unsafe { *instance = handle };
    crate::logger::info(format_args!("created instance for {app_name}"));
    SUCCESS
}

pub(super) unsafe extern "system" fn xrDestroyInstance(_instance: xr::Instance) -> xr::Result {
    window::destroy();
    let mut state = lock_runtime();
    state.session = None;
    state.swapchains.clear();
    state.spaces.clear();
    state.events.clear();
    state.instance = None;
    SUCCESS
}

pub(super) unsafe extern "system" fn xrGetInstanceProperties(
    _instance: xr::Instance,
    properties: *mut xr::InstanceProperties,
) -> xr::Result {
    if properties.is_null() {
        return VALIDATION_FAILURE;
    }
    let mut result = xr::InstanceProperties {
        ty: xr::StructureType::INSTANCE_PROPERTIES,
        next: ptr::null_mut(),
        runtime_version: xr::Version::new(0, 1, 0),
        runtime_name: [0; xr::MAX_RUNTIME_NAME_SIZE],
    };
    unsafe { copy_chars(&mut result.runtime_name, "OpenXR Simulator (Rust)") };
    unsafe { *properties = result };
    SUCCESS
}

pub(super) unsafe extern "system" fn xrGetSystem(
    _instance: xr::Instance,
    info: *const xr::SystemGetInfo,
    system_id: *mut xr::SystemId,
) -> xr::Result {
    if info.is_null() || system_id.is_null() {
        return VALIDATION_FAILURE;
    }
    if unsafe { (*info).form_factor } != xr::FormFactor::HEAD_MOUNTED_DISPLAY {
        return FORM_FACTOR_UNSUPPORTED;
    }
    unsafe { *system_id = xr::SystemId::from_raw(1) };
    SUCCESS
}

pub(super) unsafe extern "system" fn xrGetSystemProperties(
    _instance: xr::Instance,
    _system_id: xr::SystemId,
    properties: *mut xr::SystemProperties,
) -> xr::Result {
    if properties.is_null() {
        return VALIDATION_FAILURE;
    }
    let mut result = xr::SystemProperties {
        ty: xr::StructureType::SYSTEM_PROPERTIES,
        next: ptr::null_mut(),
        system_id: xr::SystemId::from_raw(1),
        vendor_id: 0,
        system_name: [0; xr::MAX_SYSTEM_NAME_SIZE],
        graphics_properties: xr::SystemGraphicsProperties {
            max_swapchain_image_width: 4096,
            max_swapchain_image_height: 4096,
            max_layer_count: 16,
        },
        tracking_properties: xr::SystemTrackingProperties {
            orientation_tracking: xr::TRUE,
            position_tracking: xr::TRUE,
        },
    };
    unsafe { copy_chars(&mut result.system_name, "OpenXR Simulator Desktop HMD") };
    unsafe { *properties = result };
    SUCCESS
}

pub(super) unsafe extern "system" fn xrEnumerateViewConfigurations(
    _instance: xr::Instance,
    _system_id: xr::SystemId,
    capacity: u32,
    count: *mut u32,
    configurations: *mut xr::ViewConfigurationType,
) -> xr::Result {
    unsafe {
        write_count_buffer(
            capacity,
            count,
            &[xr::ViewConfigurationType::PRIMARY_STEREO],
            configurations,
        )
    }
}

pub(super) unsafe extern "system" fn xrGetViewConfigurationProperties(
    _instance: xr::Instance,
    _system_id: xr::SystemId,
    _configuration: xr::ViewConfigurationType,
    properties: *mut xr::ViewConfigurationProperties,
) -> xr::Result {
    if properties.is_null() {
        return VALIDATION_FAILURE;
    }
    unsafe {
        *properties = xr::ViewConfigurationProperties {
            ty: xr::StructureType::VIEW_CONFIGURATION_PROPERTIES,
            next: ptr::null_mut(),
            view_configuration_type: _configuration,
            fov_mutable: xr::FALSE,
        };
    }
    SUCCESS
}

pub(super) unsafe extern "system" fn xrEnumerateViewConfigurationViews(
    _instance: xr::Instance,
    _system_id: xr::SystemId,
    _configuration: xr::ViewConfigurationType,
    capacity: u32,
    count: *mut u32,
    views: *mut xr::ViewConfigurationView,
) -> xr::Result {
    if !count.is_null() {
        unsafe { *count = 2 };
    }
    if capacity == 0 || views.is_null() {
        return SUCCESS;
    }
    let settings = config::snapshot();
    for index in 0..capacity.min(2) {
        unsafe {
            *views.add(index as usize) = xr::ViewConfigurationView {
                ty: xr::StructureType::VIEW_CONFIGURATION_VIEW,
                next: ptr::null_mut(),
                recommended_image_rect_width: settings.display.render_width,
                recommended_image_rect_height: settings.display.render_height,
                recommended_swapchain_sample_count: 1,
                max_image_rect_width: 4096,
                max_image_rect_height: 4096,
                max_swapchain_sample_count: 1,
            };
        }
    }
    SUCCESS
}

pub(super) unsafe extern "system" fn xrEnumerateEnvironmentBlendModes(
    _instance: xr::Instance,
    _system_id: xr::SystemId,
    _configuration: xr::ViewConfigurationType,
    capacity: u32,
    count: *mut u32,
    blend_modes: *mut xr::EnvironmentBlendMode,
) -> xr::Result {
    unsafe {
        write_count_buffer(
            capacity,
            count,
            &[xr::EnvironmentBlendMode::OPAQUE],
            blend_modes,
        )
    }
}

pub(super) unsafe extern "system" fn xrGetD3D11GraphicsRequirementsKHR(
    _instance: xr::Instance,
    _system_id: xr::SystemId,
    requirements: *mut xr::GraphicsRequirementsD3D11KHR,
) -> xr::Result {
    if requirements.is_null() {
        return VALIDATION_FAILURE;
    }
    let factory: IDXGIFactory1 = match unsafe { CreateDXGIFactory1() } {
        Ok(factory) => factory,
        Err(error) => {
            crate::logger::error(format_args!("CreateDXGIFactory1 failed: {error}"));
            return RUNTIME_FAILURE;
        }
    };
    let mut selected = None;
    for index in 0..16 {
        let Ok(adapter) = (unsafe { factory.EnumAdapters1(index) }) else {
            break;
        };
        let Ok(description) = (unsafe { adapter.GetDesc1() }) else {
            continue;
        };
        if description.Flags & 2 != 0 {
            // DXGI_ADAPTER_FLAG_SOFTWARE
            continue;
        }
        selected = Some(description);
        break;
    }
    let Some(description) = selected else {
        return SYSTEM_INVALID;
    };
    unsafe {
        *requirements = xr::GraphicsRequirementsD3D11KHR {
            ty: xr::StructureType::GRAPHICS_REQUIREMENTS_D3D11_KHR,
            next: ptr::null_mut(),
            adapter_luid: xr::platform::LUID {
                LowPart: description.AdapterLuid.LowPart,
                HighPart: description.AdapterLuid.HighPart,
            },
            min_feature_level: D3D_FEATURE_LEVEL_11_0.0,
        };
    }
    SUCCESS
}
