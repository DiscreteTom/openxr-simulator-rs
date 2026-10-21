use openxr::{
    ApplicationInfo, CompositionLayerProjection, CompositionLayerProjectionView, D3D11, Entry,
    EnvironmentBlendMode, FormFactor, Posef, ReferenceSpaceType, SwapchainCreateInfo,
    SwapchainSubImage, SwapchainUsageFlags, ViewConfigurationType,
};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
    ID3D11DeviceContext,
};
use windows::core::Interface;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let entry = unsafe { Entry::load(&())? };
    let extensions = entry.enumerate_extensions()?;
    assert!(
        extensions.khr_d3d11_enable,
        "runtime did not advertise XR_KHR_D3D11_enable"
    );

    let instance = entry.create_instance(
        &ApplicationInfo {
            application_name: "OpenXR Simulator Probe",
            application_version: 1,
            engine_name: "probe",
            engine_version: 1,
            api_version: openxr::Version::new(1, 0, 0),
        },
        &extensions,
        &[],
        &(),
    )?;
    let system = instance.system(FormFactor::HEAD_MOUNTED_DISPLAY)?;
    let configurations = instance.enumerate_view_configurations(system)?;
    assert!(configurations.contains(&ViewConfigurationType::PRIMARY_STEREO));
    let views = instance
        .enumerate_view_configuration_views(system, ViewConfigurationType::PRIMARY_STEREO)?;
    assert_eq!(views.len(), 2, "stereo must expose exactly two views");

    let _requirements = instance.graphics_requirements::<D3D11>(system)?;
    let mut device: Option<ID3D11Device> = None;
    let mut context: Option<ID3D11DeviceContext> = None;
    unsafe {
        D3D11CreateDevice(
            None,
            windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_FLAG(0),
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )?;
    }
    let device = device.expect("D3D11 device");
    let (session, mut waiter, mut stream) = unsafe {
        instance.create_session::<D3D11>(
            system,
            &openxr::d3d::SessionCreateInfoD3D11 {
                device: Interface::as_raw(&device),
            },
        )?
    };
    session.begin(ViewConfigurationType::PRIMARY_STEREO)?;
    let space = session.create_reference_space(ReferenceSpaceType::LOCAL, Posef::IDENTITY)?;
    let width = views[0].recommended_image_rect_width;
    let height = views[0].recommended_image_rect_height;
    let mut swapchain = session.create_swapchain(&SwapchainCreateInfo {
        create_flags: Default::default(),
        usage_flags: SwapchainUsageFlags::COLOR_ATTACHMENT | SwapchainUsageFlags::SAMPLED,
        format: 91, // DXGI_FORMAT_B8G8R8A8_UNORM_SRGB
        sample_count: 1,
        width,
        height,
        face_count: 1,
        array_size: 2,
        mip_count: 1,
    })?;

    for _ in 0..5 {
        let frame = waiter.wait()?;
        let image_index = swapchain.acquire_image()?;
        swapchain.wait_image(openxr::Duration::INFINITE)?;
        stream.begin()?;
        if frame.should_render {
            let (_, located) = session.locate_views(
                ViewConfigurationType::PRIMARY_STEREO,
                frame.predicted_display_time,
                &space,
            )?;
            let projection_views = [
                CompositionLayerProjectionView::new()
                    .pose(located[0].pose)
                    .fov(located[0].fov)
                    .sub_image(
                        SwapchainSubImage::new()
                            .swapchain(&swapchain)
                            .image_array_index(0)
                            .image_rect(openxr::Rect2Di {
                                offset: openxr::Offset2Di { x: 0, y: 0 },
                                extent: openxr::Extent2Di {
                                    width: width as i32,
                                    height: height as i32,
                                },
                            }),
                    ),
                CompositionLayerProjectionView::new()
                    .pose(located[1].pose)
                    .fov(located[1].fov)
                    .sub_image(
                        SwapchainSubImage::new()
                            .swapchain(&swapchain)
                            .image_array_index(1)
                            .image_rect(openxr::Rect2Di {
                                offset: openxr::Offset2Di { x: 0, y: 0 },
                                extent: openxr::Extent2Di {
                                    width: width as i32,
                                    height: height as i32,
                                },
                            }),
                    ),
            ];
            let projection = CompositionLayerProjection::new()
                .space(&space)
                .views(&projection_views);
            let layers: [&openxr::CompositionLayerBase<'_, D3D11>; 1] = [&projection];
            stream.end(
                frame.predicted_display_time,
                EnvironmentBlendMode::OPAQUE,
                &layers,
            )?;
        } else {
            stream.end(
                frame.predicted_display_time,
                EnvironmentBlendMode::OPAQUE,
                &[],
            )?;
        }
        swapchain.release_image()?;
        let _ = image_index;
    }
    session.end()?;
    println!(
        "runtime OK: {} views at {}x{}, D3D11 session and frame submission succeeded",
        views.len(),
        width,
        height
    );
    Ok(())
}
