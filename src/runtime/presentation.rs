use super::*;

use std::time::Duration;

use openxr_sys::Handle;

use crate::{config, d3d11_preview, window};

pub(super) unsafe extern "system" fn xrEndFrame(
    _session: xr::Session,
    end_info: *const xr::FrameEndInfo,
) -> xr::Result {
    if end_info.is_null() {
        return VALIDATION_FAILURE;
    }
    let info = unsafe { &*end_info };
    let mut state = lock_runtime();
    state.update_fps_and_title();
    if info.layer_count == 0 || info.layers.is_null() {
        return SUCCESS;
    }

    let settings = config::snapshot();
    let rate = settings.display.mirror_rate_hz;
    if rate > 0
        && state
            .last_mirror
            .is_some_and(|last| last.elapsed() < Duration::from_secs_f32(1.0 / rate as f32))
    {
        return SUCCESS;
    }
    state.last_mirror = Some(Instant::now());

    let mut projection: Option<&xr::CompositionLayerProjection> = None;
    for index in 0..info.layer_count {
        let header = unsafe { *info.layers.add(index as usize) };
        if header.is_null() {
            continue;
        }
        if unsafe { (*header).ty } == xr::StructureType::COMPOSITION_LAYER_PROJECTION {
            projection = Some(unsafe { &*header.cast() });
            break;
        }
    }
    let Some(projection) = projection else {
        return SUCCESS;
    };
    if projection.views.is_null() || projection.view_count == 0 {
        return SUCCESS;
    }

    let RuntimeState {
        session,
        swapchains,
        ..
    } = &mut *state;
    let Some(session) = session.as_mut() else {
        return HANDLE_INVALID;
    };
    let mut eye_sources = Vec::new();
    for eye_index in 0..projection.view_count.min(2) {
        let view = unsafe { &*projection.views.add(eye_index as usize) };
        let Some(chain) = swapchains.get(&view.sub_image.swapchain.into_raw()) else {
            continue;
        };
        if chain.is_depth {
            continue;
        }
        let image_index = chain.last_released.or(chain.acquired).unwrap_or(0) as usize;
        let Some(texture) = chain.images.get(image_index) else {
            continue;
        };
        eye_sources.push((
            texture,
            view.sub_image.image_array_index,
            view.sub_image.image_rect,
        ));
    }

    if !session.preview_attempted {
        session.preview_attempted = true;
        match d3d11_preview::Preview::new(session.mirror.device(), window::hwnd()) {
            Ok(preview) => {
                crate::logger::info(format_args!("GPU preview initialized"));
                session.preview = Some(preview);
            }
            Err(error) => {
                crate::logger::error(format_args!(
                    "GPU preview initialization failed; using CPU readback: {error}"
                ));
            }
        }
    }
    if let Some(preview) = session.preview.as_mut() {
        let frames: Vec<d3d11_preview::EyeFrame<'_>> = eye_sources
            .iter()
            .map(
                |(texture, array_index, image_rect)| d3d11_preview::EyeFrame {
                    texture,
                    array_index: *array_index,
                    image_rect: *image_rect,
                },
            )
            .collect();
        match preview.present(&frames, settings.display.view_mode) {
            Ok(()) => return SUCCESS,
            Err(error) => {
                crate::logger::error(format_args!(
                    "GPU preview failed; switching to CPU readback: {error}"
                ));
                session.preview = None;
            }
        }
    }

    let mut eyes: Vec<(Vec<u8>, u32, u32)> = Vec::new();
    for (texture, array_index, image_rect) in eye_sources {
        match session.mirror.read_eye(texture, array_index, image_rect) {
            Ok((pixels, width, height)) => eyes.push((pixels, width, height)),
            Err(error) => {
                crate::logger::error(format_args!("readback failed: {error}"));
            }
        }
    }
    if !eyes.is_empty() {
        let borrowed: Vec<(&[u8], u32, u32)> = eyes
            .iter()
            .map(|(pixels, width, height)| (pixels.as_slice(), *width, *height))
            .collect();
        window::publish_composite(&borrowed, settings.display.view_mode);
    }
    SUCCESS
}
