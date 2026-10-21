use std::ptr;

use openxr_sys as xr;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D::Fxc::{D3DCOMPILE_OPTIMIZATION_LEVEL3, D3DCompile};
use windows::Win32::Graphics::Direct3D::{
    D3D_SRV_DIMENSION_TEXTURE2DARRAY, D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP, ID3DBlob,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_CONSTANT_BUFFER, D3D11_CPU_ACCESS_WRITE, D3D11_CULL_NONE, D3D11_FILL_SOLID,
    D3D11_FILTER_MIN_MAG_MIP_LINEAR, D3D11_MAP_WRITE_DISCARD, D3D11_RASTERIZER_DESC,
    D3D11_RTV_DIMENSION_TEXTURE2D, D3D11_SAMPLER_DESC, D3D11_SHADER_RESOURCE_VIEW_DESC,
    D3D11_TEX2D_ARRAY_SRV, D3D11_TEX2D_RTV, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DYNAMIC,
    D3D11_VIEWPORT, ID3D11Buffer, ID3D11Device, ID3D11DeviceContext, ID3D11PixelShader,
    ID3D11RasterizerState, ID3D11RenderTargetView, ID3D11SamplerState, ID3D11ShaderResourceView,
    ID3D11Texture2D, ID3D11VertexShader,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
    DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_PRESENT, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_DISCARD,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice, IDXGIFactory2, IDXGISwapChain1,
};
use windows::core::{Interface, PCSTR};

use crate::aspect;
use crate::config::ViewMode;
use crate::window;

const SHADER_SOURCE: &str = r#"
struct VsOut {
    float4 position : SV_POSITION;
    float2 uv : TEXCOORD0;
};

VsOut VSMain(uint vertexId : SV_VertexID) {
    float2 position = float2(
        (vertexId == 1 || vertexId == 3) ? 1.0 : -1.0,
        vertexId >= 2 ? -1.0 : 1.0
    );
    float2 uv = float2(
        (vertexId == 1 || vertexId == 3) ? 1.0 : 0.0,
        vertexId >= 2 ? 1.0 : 0.0
    );
    VsOut output;
    output.position = float4(position, 0.0, 1.0);
    output.uv = uv;
    return output;
}

Texture2D eyeTexture : register(t0);
SamplerState linearSampler : register(s0);
cbuffer UvRect : register(b0) {
    float4 sourceRect;
};

float4 PSMain(VsOut input) : SV_TARGET {
    float2 uv = float2(
        lerp(sourceRect.x, sourceRect.z, input.uv.x),
        lerp(sourceRect.y, sourceRect.w, input.uv.y)
    );
    return eyeTexture.Sample(linearSampler, uv);
}
"#;

#[repr(C)]
#[derive(Clone, Copy)]
struct UvRect {
    values: [f32; 4],
}

pub struct Preview {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    swapchain: IDXGISwapChain1,
    vertex_shader: ID3D11VertexShader,
    pixel_shader: ID3D11PixelShader,
    sampler: ID3D11SamplerState,
    rasterizer: ID3D11RasterizerState,
    constant_buffer: ID3D11Buffer,
    width: u32,
    height: u32,
}

impl Preview {
    pub fn new(device: &ID3D11Device, hwnd: HWND) -> windows::core::Result<Self> {
        let context = unsafe { device.GetImmediateContext() }?;
        let (width, height) = window::client_size();
        let swapchain = create_swapchain(device, hwnd, width, height)?;
        let (vertex_shader, pixel_shader) = compile_shaders(device)?;
        let sampler = create_sampler(device)?;
        let rasterizer = create_rasterizer(device)?;
        let constant_buffer = create_constant_buffer(device)?;
        Ok(Self {
            device: device.clone(),
            context,
            swapchain,
            vertex_shader,
            pixel_shader,
            sampler,
            rasterizer,
            constant_buffer,
            width,
            height,
        })
    }

    pub fn present(&mut self, eyes: &[EyeFrame<'_>], mode: ViewMode) -> windows::core::Result<()> {
        let (width, height) = window::client_size();
        if width != self.width || height != self.height {
            unsafe {
                self.context.OMSetRenderTargets(None, None);
                self.swapchain.ResizeBuffers(
                    0,
                    width,
                    height,
                    DXGI_FORMAT_UNKNOWN,
                    Default::default(),
                )?;
            }
            self.width = width;
            self.height = height;
        }

        let backbuffer: ID3D11Texture2D = unsafe { self.swapchain.GetBuffer(0) }?;
        let selected: Vec<&EyeFrame<'_>> = match mode {
            ViewMode::Stereo => eyes.iter().collect(),
            ViewMode::LeftEye => eyes.first().into_iter().collect(),
            ViewMode::RightEye => eyes.get(1).into_iter().collect(),
        };
        if selected.is_empty() {
            return Ok(());
        }

        let srgb = selected.iter().any(|eye| is_srgb_texture(eye.texture));
        let rtv = create_render_target_view(&self.device, &backbuffer, srgb)?;
        let mut deferred_context = None;
        unsafe {
            self.device
                .CreateDeferredContext(0, Some(&mut deferred_context))?
        };
        let deferred = deferred_context.ok_or_else(windows::core::Error::from_thread)?;
        let slots: Vec<Option<ID3D11RenderTargetView>> = vec![Some(rtv.clone())];
        unsafe {
            deferred.ClearRenderTargetView(&rtv, &[0.015, 0.015, 0.025, 1.0]);
            deferred.OMSetRenderTargets(Some(&slots), None);
            deferred.IASetInputLayout(None);
            deferred.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
            deferred.VSSetShader(&self.vertex_shader, None);
            deferred.PSSetShader(&self.pixel_shader, None);
            deferred.RSSetState(&self.rasterizer);
            deferred.OMSetDepthStencilState(None, 0);
            deferred.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
            deferred.PSSetConstantBuffers(0, Some(&[Some(self.constant_buffer.clone())]));
        }

        let (source_width, source_height) = selected
            .first()
            .map(|eye| eye.content_size())
            .unwrap_or((1.0, 1.0));
        let viewports = layout_viewports(
            width,
            height,
            selected.len(),
            mode,
            source_width,
            source_height,
        );
        for (index, eye) in selected.iter().enumerate() {
            let Some(viewport) = viewports.get(index) else {
                continue;
            };
            let srv = create_eye_srv(&self.device, eye.texture, eye.array_index)?;
            let uv = eye.uv_rect();
            let mut mapped = Default::default();
            unsafe {
                deferred.Map(
                    &self.constant_buffer,
                    0,
                    D3D11_MAP_WRITE_DISCARD,
                    0,
                    Some(&mut mapped),
                )?;
                ptr::write_unaligned(mapped.pData.cast::<UvRect>(), UvRect { values: uv });
                deferred.Unmap(&self.constant_buffer, 0);
                deferred.RSSetViewports(Some(std::slice::from_ref(viewport)));
                deferred.PSSetShaderResources(0, Some(&[Some(srv.clone())]));
                deferred.Draw(4, 0);
            }
        }

        let null_srvs = [None];
        unsafe {
            deferred.PSSetShaderResources(0, Some(&null_srvs));
        }
        let mut command_list = None;
        unsafe { deferred.FinishCommandList(false, Some(&mut command_list)) }?;
        let command_list = command_list.ok_or_else(windows::core::Error::from_thread)?;
        unsafe {
            self.context.ExecuteCommandList(&command_list, true);
        }
        unsafe { self.swapchain.Present(0, DXGI_PRESENT(0)) }.ok()?;
        Ok(())
    }
}

pub struct EyeFrame<'a> {
    pub texture: &'a ID3D11Texture2D,
    pub array_index: u32,
    pub image_rect: xr::Rect2Di,
}

impl EyeFrame<'_> {
    fn content_size(&self) -> (f32, f32) {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { self.texture.GetDesc(&mut desc) };
        if self.image_rect.extent.width > 0 && self.image_rect.extent.height > 0 {
            (
                self.image_rect.extent.width as f32,
                self.image_rect.extent.height as f32,
            )
        } else {
            (desc.Width as f32, desc.Height as f32)
        }
    }

    fn uv_rect(&self) -> [f32; 4] {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { self.texture.GetDesc(&mut desc) };
        let full = [0.0, 0.0, 1.0, 1.0];
        if desc.Width == 0
            || desc.Height == 0
            || self.image_rect.extent.width <= 0
            || self.image_rect.extent.height <= 0
        {
            return full;
        }
        let left = self.image_rect.offset.x.max(0) as f32 / desc.Width as f32;
        let top = self.image_rect.offset.y.max(0) as f32 / desc.Height as f32;
        let right = (self.image_rect.offset.x + self.image_rect.extent.width).max(0) as f32
            / desc.Width as f32;
        let bottom = (self.image_rect.offset.y + self.image_rect.extent.height).max(0) as f32
            / desc.Height as f32;
        [
            left.clamp(0.0, 1.0),
            top.clamp(0.0, 1.0),
            right.clamp(0.0, 1.0),
            bottom.clamp(0.0, 1.0),
        ]
    }
}

fn create_swapchain(
    device: &ID3D11Device,
    hwnd: HWND,
    width: u32,
    height: u32,
) -> windows::core::Result<IDXGISwapChain1> {
    let dxgi_device: IDXGIDevice = device.cast()?;
    let adapter = unsafe { dxgi_device.GetAdapter() }?;
    let factory: IDXGIFactory2 = unsafe { adapter.GetParent() }?;
    let description = DXGI_SWAP_CHAIN_DESC1 {
        Width: width,
        Height: height,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        Scaling: DXGI_SCALING_STRETCH,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
        AlphaMode: DXGI_ALPHA_MODE_IGNORE,
        Flags: 0,
    };
    unsafe { factory.CreateSwapChainForHwnd(device, hwnd, &description, None, None) }
}

fn compile_shaders(
    device: &ID3D11Device,
) -> windows::core::Result<(ID3D11VertexShader, ID3D11PixelShader)> {
    let vertex = compile_shader(b"VSMain\0", b"vs_5_0\0")?;
    let pixel = compile_shader(b"PSMain\0", b"ps_5_0\0")?;
    let mut vertex_shader = None;
    let mut pixel_shader = None;
    unsafe {
        device.CreateVertexShader(blob_bytes(&vertex), None, Some(&mut vertex_shader))?;
        device.CreatePixelShader(blob_bytes(&pixel), None, Some(&mut pixel_shader))?;
    }
    Ok((
        vertex_shader.ok_or_else(windows::core::Error::from_thread)?,
        pixel_shader.ok_or_else(windows::core::Error::from_thread)?,
    ))
}

fn compile_shader(entry: &[u8], target: &[u8]) -> windows::core::Result<ID3DBlob> {
    let mut code = None;
    let mut errors = None;
    unsafe {
        D3DCompile(
            SHADER_SOURCE.as_ptr().cast(),
            SHADER_SOURCE.len(),
            PCSTR::null(),
            None,
            None,
            PCSTR(entry.as_ptr()),
            PCSTR(target.as_ptr()),
            D3DCOMPILE_OPTIMIZATION_LEVEL3,
            0,
            &mut code,
            Some(&mut errors),
        )?;
    }
    code.ok_or_else(windows::core::Error::from_thread)
}

fn blob_bytes(blob: &ID3DBlob) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(blob.GetBufferPointer().cast::<u8>(), blob.GetBufferSize())
    }
}

fn create_sampler(device: &ID3D11Device) -> windows::core::Result<ID3D11SamplerState> {
    let description = D3D11_SAMPLER_DESC {
        Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
        AddressU: windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressV: windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressW: windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE_ADDRESS_CLAMP,
        ComparisonFunc: windows::Win32::Graphics::Direct3D11::D3D11_COMPARISON_NEVER,
        MinLOD: 0.0,
        MaxLOD: f32::MAX,
        ..Default::default()
    };
    let mut sampler = None;
    unsafe { device.CreateSamplerState(&description, Some(&mut sampler)) }?;
    sampler.ok_or_else(windows::core::Error::from_thread)
}

fn create_rasterizer(device: &ID3D11Device) -> windows::core::Result<ID3D11RasterizerState> {
    let description = D3D11_RASTERIZER_DESC {
        FillMode: D3D11_FILL_SOLID,
        CullMode: D3D11_CULL_NONE,
        DepthClipEnable: true.into(),
        ..Default::default()
    };
    let mut rasterizer = None;
    unsafe { device.CreateRasterizerState(&description, Some(&mut rasterizer)) }?;
    rasterizer.ok_or_else(windows::core::Error::from_thread)
}

fn create_constant_buffer(device: &ID3D11Device) -> windows::core::Result<ID3D11Buffer> {
    let description = windows::Win32::Graphics::Direct3D11::D3D11_BUFFER_DESC {
        ByteWidth: std::mem::size_of::<UvRect>() as u32,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
        ..Default::default()
    };
    let mut buffer = None;
    unsafe { device.CreateBuffer(&description, None, Some(&mut buffer)) }?;
    buffer.ok_or_else(windows::core::Error::from_thread)
}

fn create_render_target_view(
    device: &ID3D11Device,
    texture: &ID3D11Texture2D,
    srgb: bool,
) -> windows::core::Result<ID3D11RenderTargetView> {
    let description = windows::Win32::Graphics::Direct3D11::D3D11_RENDER_TARGET_VIEW_DESC {
        Format: if srgb {
            DXGI_FORMAT_B8G8R8A8_UNORM_SRGB
        } else {
            DXGI_FORMAT_B8G8R8A8_UNORM
        },
        ViewDimension: D3D11_RTV_DIMENSION_TEXTURE2D,
        Anonymous: windows::Win32::Graphics::Direct3D11::D3D11_RENDER_TARGET_VIEW_DESC_0 {
            Texture2D: D3D11_TEX2D_RTV { MipSlice: 0 },
        },
    };
    let mut view = None;
    unsafe { device.CreateRenderTargetView(texture, Some(&description), Some(&mut view)) }?;
    view.ok_or_else(windows::core::Error::from_thread)
}

fn create_eye_srv(
    device: &ID3D11Device,
    texture: &ID3D11Texture2D,
    array_index: u32,
) -> windows::core::Result<ID3D11ShaderResourceView> {
    let mut texture_desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { texture.GetDesc(&mut texture_desc) };
    let description = D3D11_SHADER_RESOURCE_VIEW_DESC {
        Format: texture_desc.Format,
        ViewDimension: D3D_SRV_DIMENSION_TEXTURE2DARRAY,
        Anonymous: windows::Win32::Graphics::Direct3D11::D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
            Texture2DArray: D3D11_TEX2D_ARRAY_SRV {
                MostDetailedMip: 0,
                MipLevels: 1,
                FirstArraySlice: array_index,
                ArraySize: 1,
            },
        },
    };
    let mut view = None;
    unsafe { device.CreateShaderResourceView(texture, Some(&description), Some(&mut view)) }?;
    view.ok_or_else(windows::core::Error::from_thread)
}

fn is_srgb_texture(texture: &ID3D11Texture2D) -> bool {
    let mut description = D3D11_TEXTURE2D_DESC::default();
    unsafe { texture.GetDesc(&mut description) };
    matches!(
        description.Format,
        DXGI_FORMAT_B8G8R8A8_UNORM_SRGB | DXGI_FORMAT_R8G8B8A8_UNORM_SRGB
    )
}

fn layout_viewports(
    width: u32,
    height: u32,
    count: usize,
    mode: ViewMode,
    source_width: f32,
    source_height: f32,
) -> Vec<D3D11_VIEWPORT> {
    let content_width = if mode == ViewMode::Stereo && count >= 2 {
        source_width * 2.0
    } else {
        source_width
    };
    let fitted = aspect::fit_rect(content_width, source_height, width as f32, height as f32);
    if mode == ViewMode::Stereo && count >= 2 {
        let half = fitted.width * 0.5;
        vec![
            D3D11_VIEWPORT {
                TopLeftX: fitted.x,
                TopLeftY: fitted.y,
                Width: half,
                Height: fitted.height,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            },
            D3D11_VIEWPORT {
                TopLeftX: fitted.x + half,
                TopLeftY: fitted.y,
                Width: half,
                Height: fitted.height,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            },
        ]
    } else {
        vec![D3D11_VIEWPORT {
            TopLeftX: fitted.x,
            TopLeftY: fitted.y,
            Width: fitted.width,
            Height: fitted.height,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_viewports_split_the_client_area() {
        let viewports = layout_viewports(1000, 600, 2, ViewMode::Stereo, 1000.0, 1000.0);
        assert_eq!(viewports.len(), 2);
        assert_eq!(viewports[0].TopLeftX, 0.0);
        assert_eq!(viewports[0].TopLeftY, 50.0);
        assert_eq!(viewports[0].Width, 500.0);
        assert_eq!(viewports[0].Height, 500.0);
        assert_eq!(viewports[1].TopLeftX, 500.0);
    }

    #[test]
    fn single_eye_keeps_its_aspect_ratio() {
        let viewports = layout_viewports(1000, 600, 1, ViewMode::LeftEye, 1000.0, 2000.0);
        assert_eq!(viewports[0].TopLeftX, 350.0);
        assert_eq!(viewports[0].TopLeftY, 0.0);
        assert_eq!(viewports[0].Width, 300.0);
        assert_eq!(viewports[0].Height, 600.0);
    }
}
