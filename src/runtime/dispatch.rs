use super::*;

use std::ffi::{CStr, c_void};

use super::{actions::*, instance::*, presentation::*, session::*, spaces::*, utility::*};

pub(super) unsafe extern "system" fn xrGetInstanceProcAddr(
    _instance: xr::Instance,
    name: *const c_char,
    function: *mut Option<xr::pfn::VoidFunction>,
) -> xr::Result {
    if name.is_null() || function.is_null() {
        return VALIDATION_FAILURE;
    }
    let name = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(name) => name,
        Err(_) => return VALIDATION_FAILURE,
    };

    macro_rules! function {
        ($literal:literal, $implementation:ident) => {
            if name == $literal {
                unsafe {
                    *function = Some(std::mem::transmute::<
                        unsafe extern "system" fn() -> xr::Result,
                        xr::pfn::VoidFunction,
                    >(
                        $implementation as unsafe extern "system" fn() -> xr::Result,
                    ));
                }
                return SUCCESS;
            }
        };
        ($literal:literal, $implementation:ident, other) => {
            if name == $literal {
                unsafe {
                    *function = Some(std::mem::transmute::<*const c_void, xr::pfn::VoidFunction>(
                        $implementation as *const c_void,
                    ));
                }
                return SUCCESS;
            }
        };
    }

    function!("xrGetInstanceProcAddr", xrGetInstanceProcAddr, other);
    function!(
        "xrEnumerateApiLayerProperties",
        xrEnumerateApiLayerProperties,
        other
    );
    function!(
        "xrEnumerateInstanceExtensionProperties",
        xrEnumerateInstanceExtensionProperties,
        other
    );
    function!("xrCreateInstance", xrCreateInstance, other);
    function!("xrDestroyInstance", xrDestroyInstance, other);
    function!("xrGetInstanceProperties", xrGetInstanceProperties, other);
    function!("xrGetSystem", xrGetSystem, other);
    function!("xrGetSystemProperties", xrGetSystemProperties, other);
    function!(
        "xrEnumerateViewConfigurations",
        xrEnumerateViewConfigurations,
        other
    );
    function!(
        "xrEnumerateViewConfigurationViews",
        xrEnumerateViewConfigurationViews,
        other
    );
    function!(
        "xrGetViewConfigurationProperties",
        xrGetViewConfigurationProperties,
        other
    );
    function!(
        "xrEnumerateEnvironmentBlendModes",
        xrEnumerateEnvironmentBlendModes,
        other
    );
    function!("xrCreateSession", xrCreateSession, other);
    function!("xrDestroySession", xrDestroySession, other);
    function!(
        "xrGetD3D11GraphicsRequirementsKHR",
        xrGetD3D11GraphicsRequirementsKHR,
        other
    );
    function!(
        "xrEnumerateSwapchainFormats",
        xrEnumerateSwapchainFormats,
        other
    );
    function!("xrCreateSwapchain", xrCreateSwapchain, other);
    function!("xrDestroySwapchain", xrDestroySwapchain, other);
    function!(
        "xrEnumerateSwapchainImages",
        xrEnumerateSwapchainImages,
        other
    );
    function!("xrAcquireSwapchainImage", xrAcquireSwapchainImage, other);
    function!("xrWaitSwapchainImage", xrWaitSwapchainImage, other);
    function!("xrReleaseSwapchainImage", xrReleaseSwapchainImage, other);
    function!("xrBeginSession", xrBeginSession, other);
    function!("xrEndSession", xrEndSession, other);
    function!("xrRequestExitSession", xrRequestExitSession, other);
    function!("xrWaitFrame", xrWaitFrame, other);
    function!("xrBeginFrame", xrBeginFrame, other);
    function!("xrEndFrame", xrEndFrame, other);
    function!("xrPollEvent", xrPollEvent, other);
    function!("xrLocateViews", xrLocateViews, other);
    function!(
        "xrEnumerateReferenceSpaces",
        xrEnumerateReferenceSpaces,
        other
    );
    function!("xrCreateReferenceSpace", xrCreateReferenceSpace, other);
    function!("xrDestroySpace", xrDestroySpace, other);
    function!("xrLocateSpace", xrLocateSpace, other);
    function!("xrCreateActionSpace", xrCreateActionSpace, other);
    function!("xrCreateActionSet", xrCreateActionSet, other);
    function!("xrDestroyActionSet", xrDestroyActionSet, other);
    function!("xrCreateAction", xrCreateAction, other);
    function!("xrDestroyAction", xrDestroyAction, other);
    function!(
        "xrSuggestInteractionProfileBindings",
        xrSuggestInteractionProfileBindings,
        other
    );
    function!(
        "xrAttachSessionActionSets",
        xrAttachSessionActionSets,
        other
    );
    function!("xrGetActionStateBoolean", xrGetActionStateBoolean, other);
    function!("xrGetActionStateFloat", xrGetActionStateFloat, other);
    function!("xrGetActionStatePose", xrGetActionStatePose, other);
    function!("xrGetActionStateVector2f", xrGetActionStateVector2f, other);
    function!("xrSyncActions", xrSyncActions, other);
    function!("xrStringToPath", xrStringToPath, other);
    function!("xrPathToString", xrPathToString, other);
    function!(
        "xrGetCurrentInteractionProfile",
        xrGetCurrentInteractionProfile,
        other
    );
    function!(
        "xrEnumerateBoundSourcesForAction",
        xrEnumerateBoundSourcesForAction,
        other
    );
    function!(
        "xrGetInputSourceLocalizedName",
        xrGetInputSourceLocalizedName,
        other
    );
    function!("xrApplyHapticFeedback", xrApplyHapticFeedback, other);
    function!("xrStopHapticFeedback", xrStopHapticFeedback, other);
    function!(
        "xrGetReferenceSpaceBoundsRect",
        xrGetReferenceSpaceBoundsRect,
        other
    );
    function!("xrResultToString", xrResultToString, other);
    function!("xrStructureTypeToString", xrStructureTypeToString, other);
    function!(
        "xrConvertWin32PerformanceCounterToTimeKHR",
        xrConvertWin32PerformanceCounterToTimeKHR,
        other
    );
    function!(
        "xrConvertTimeToWin32PerformanceCounterKHR",
        xrConvertTimeToWin32PerformanceCounterKHR,
        other
    );
    FUNCTION_UNSUPPORTED
}
