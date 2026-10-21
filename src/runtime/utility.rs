use super::*;

pub(super) unsafe extern "system" fn xrResultToString(
    _instance: xr::Instance,
    value: xr::Result,
    buffer: *mut c_char,
) -> xr::Result {
    if buffer.is_null() {
        return VALIDATION_FAILURE;
    }
    let text = format!("{value:?}");
    unsafe {
        ptr::copy_nonoverlapping(
            text.as_ptr(),
            buffer.cast::<u8>(),
            text.len().min(xr::MAX_RESULT_STRING_SIZE - 1),
        );
        *buffer.add(text.len().min(xr::MAX_RESULT_STRING_SIZE - 1)) = 0;
    }
    SUCCESS
}

pub(super) unsafe extern "system" fn xrStructureTypeToString(
    _instance: xr::Instance,
    value: xr::StructureType,
    buffer: *mut c_char,
) -> xr::Result {
    if buffer.is_null() {
        return VALIDATION_FAILURE;
    }
    let text = format!("{value:?}");
    unsafe {
        ptr::copy_nonoverlapping(
            text.as_ptr(),
            buffer.cast::<u8>(),
            text.len().min(xr::MAX_STRUCTURE_NAME_SIZE - 1),
        );
        *buffer.add(text.len().min(xr::MAX_STRUCTURE_NAME_SIZE - 1)) = 0;
    }
    SUCCESS
}

pub(super) unsafe extern "system" fn xrConvertWin32PerformanceCounterToTimeKHR(
    _instance: xr::Instance,
    counter: *const i64,
    time: *mut xr::Time,
) -> xr::Result {
    if counter.is_null() || time.is_null() {
        return VALIDATION_FAILURE;
    }
    let mut frequency = 0i64;
    unsafe { windows::Win32::System::Performance::QueryPerformanceFrequency(&mut frequency) }.ok();
    if frequency <= 0 {
        return RUNTIME_FAILURE;
    }
    let ticks = unsafe { *counter };
    let nanos = (ticks as i128 * 1_000_000_000i128) / frequency as i128;
    unsafe { *time = xr::Time::from_nanos(nanos.min(i64::MAX as i128) as i64) };
    SUCCESS
}

pub(super) unsafe extern "system" fn xrConvertTimeToWin32PerformanceCounterKHR(
    _instance: xr::Instance,
    time: xr::Time,
    counter: *mut i64,
) -> xr::Result {
    if counter.is_null() {
        return VALIDATION_FAILURE;
    }
    let mut frequency = 0i64;
    unsafe { windows::Win32::System::Performance::QueryPerformanceFrequency(&mut frequency) }.ok();
    if frequency <= 0 {
        return RUNTIME_FAILURE;
    }
    unsafe {
        *counter = (time.as_nanos() as i128 * frequency as i128 / 1_000_000_000i128) as i64;
    }
    SUCCESS
}
