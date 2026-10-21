use std::ffi::c_void;

use half::f16;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_DEPTH_STENCIL, D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_BOX,
    D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_TYPELESS, DXGI_FORMAT_B8G8R8A8_UNORM,
    DXGI_FORMAT_B8G8R8A8_UNORM_SRGB, DXGI_FORMAT_D24_UNORM_S8_UINT, DXGI_FORMAT_D32_FLOAT,
    DXGI_FORMAT_R8G8B8A8_TYPELESS, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
    DXGI_FORMAT_R10G10B10A2_TYPELESS, DXGI_FORMAT_R10G10B10A2_UNORM,
    DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_FORMAT_R16G16B16A16_TYPELESS, DXGI_SAMPLE_DESC,
};
use windows::core::Interface;

use openxr_sys as xr;

pub const COLOR_FORMATS: [DXGI_FORMAT; 5] = [
    DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
    DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
    DXGI_FORMAT_B8G8R8A8_UNORM,
    DXGI_FORMAT_R8G8B8A8_UNORM,
    DXGI_FORMAT_R16G16B16A16_FLOAT,
];

pub const DEPTH_FORMATS: [DXGI_FORMAT; 2] = [DXGI_FORMAT_D24_UNORM_S8_UINT, DXGI_FORMAT_D32_FLOAT];

pub fn is_supported_format(format: i64) -> bool {
    let format = DXGI_FORMAT(format as i32);
    COLOR_FORMATS.contains(&format) || DEPTH_FORMATS.contains(&format)
}

pub fn is_depth_format(format: i64) -> bool {
    let format = DXGI_FORMAT(format as i32);
    DEPTH_FORMATS.contains(&format)
}

fn typeless(format: DXGI_FORMAT) -> DXGI_FORMAT {
    match format {
        DXGI_FORMAT_B8G8R8A8_UNORM | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB => {
            DXGI_FORMAT_B8G8R8A8_TYPELESS
        }
        DXGI_FORMAT_R8G8B8A8_UNORM | DXGI_FORMAT_R8G8B8A8_UNORM_SRGB => {
            DXGI_FORMAT_R8G8B8A8_TYPELESS
        }
        DXGI_FORMAT_R10G10B10A2_UNORM => DXGI_FORMAT_R10G10B10A2_TYPELESS,
        DXGI_FORMAT_R16G16B16A16_FLOAT => DXGI_FORMAT_R16G16B16A16_TYPELESS,
        _ => format,
    }
}

pub struct TextureSpec {
    pub format: DXGI_FORMAT,
    pub width: u32,
    pub height: u32,
    pub array_size: u32,
    pub mip_levels: u32,
    pub mutable_format: bool,
    pub depth: bool,
}

pub fn create_texture(
    device: &ID3D11Device,
    spec: &TextureSpec,
) -> windows::core::Result<ID3D11Texture2D> {
    let bind_flags = if spec.depth {
        (D3D11_BIND_DEPTH_STENCIL.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32
    } else {
        (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32
    };
    let desc = D3D11_TEXTURE2D_DESC {
        Width: spec.width.max(1),
        Height: spec.height.max(1),
        MipLevels: spec.mip_levels.max(1),
        ArraySize: spec.array_size.max(1),
        Format: if spec.mutable_format {
            typeless(spec.format)
        } else {
            spec.format
        },
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: bind_flags,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut texture = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }?;
    texture.ok_or_else(windows::core::Error::from_thread)
}

pub struct Mirror {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    staging: Option<ID3D11Texture2D>,
    staging_desc: Option<D3D11_TEXTURE2D_DESC>,
}

impl Mirror {
    pub fn new(device: ID3D11Device) -> windows::core::Result<Self> {
        let context = unsafe { device.GetImmediateContext() }?;
        Ok(Self {
            device,
            context,
            staging: None,
            staging_desc: None,
        })
    }

    pub fn device(&self) -> &ID3D11Device {
        &self.device
    }

    pub fn read_eye(
        &mut self,
        texture: &ID3D11Texture2D,
        array_index: u32,
        rect: xr::Rect2Di,
    ) -> windows::core::Result<(Vec<u8>, u32, u32)> {
        let mut source_desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { texture.GetDesc(&mut source_desc) };
        if source_desc.Format == DXGI_FORMAT_D24_UNORM_S8_UINT
            || source_desc.Format == DXGI_FORMAT_D32_FLOAT
        {
            return Err(windows::core::Error::from_thread());
        }

        let (x, y, width, height) = resolve_rect(rect, source_desc.Width, source_desc.Height);
        if width == 0 || height == 0 {
            return Err(windows::core::Error::from_thread());
        }

        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: source_desc.Format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        if self.staging_desc.as_ref() != Some(&desc) {
            let mut staging = None;
            unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut staging)) }?;
            self.staging = staging;
            self.staging_desc = Some(desc);
        }
        let staging = self
            .staging
            .as_ref()
            .ok_or_else(windows::core::Error::from_thread)?;
        let box_region = D3D11_BOX {
            left: x,
            top: y,
            front: 0,
            right: x + width,
            bottom: y + height,
            back: 1,
        };
        let source_subresource = array_index.saturating_mul(source_desc.MipLevels.max(1));
        unsafe {
            self.context.CopySubresourceRegion(
                staging,
                0,
                0,
                0,
                0,
                texture,
                source_subresource,
                Some(&box_region),
            );
        }

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.context
                .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
        }?;
        let result = unsafe {
            convert_rows(
                mapped.pData.cast(),
                mapped.RowPitch as usize,
                width,
                height,
                source_desc.Format,
            )
        };
        unsafe {
            self.context.Unmap(staging, 0);
        }
        result
    }
}

fn resolve_rect(
    rect: xr::Rect2Di,
    texture_width: u32,
    texture_height: u32,
) -> (u32, u32, u32, u32) {
    let x = rect.offset.x.max(0) as u32;
    let y = rect.offset.y.max(0) as u32;
    let mut width = rect.extent.width.max(0) as u32;
    let mut height = rect.extent.height.max(0) as u32;
    if width == 0 || height == 0 || x >= texture_width || y >= texture_height {
        return (0, 0, texture_width, texture_height);
    }
    if x + width > texture_width {
        width = texture_width - x;
    }
    if y + height > texture_height {
        height = texture_height - y;
    }
    (x, y, width, height)
}

unsafe fn convert_rows(
    source: *const u8,
    pitch: usize,
    width: u32,
    height: u32,
    format: DXGI_FORMAT,
) -> windows::core::Result<(Vec<u8>, u32, u32)> {
    let mut output = vec![0u8; width as usize * height as usize * 4];
    if source.is_null() {
        return Err(windows::core::Error::from_thread());
    }
    for row in 0..height as usize {
        let input = unsafe { source.add(row * pitch) };
        let target = &mut output[row * width as usize * 4..(row + 1) * width as usize * 4];
        if format == DXGI_FORMAT_B8G8R8A8_UNORM || format == DXGI_FORMAT_B8G8R8A8_UNORM_SRGB {
            let count = width as usize * 4;
            unsafe { std::ptr::copy_nonoverlapping(input, target.as_mut_ptr(), count) };
        } else if format == DXGI_FORMAT_R8G8B8A8_UNORM || format == DXGI_FORMAT_R8G8B8A8_UNORM_SRGB
        {
            for x in 0..width as usize {
                let p = unsafe { input.add(x * 4) };
                let o = x * 4;
                target[o] = unsafe { *p.add(2) };
                target[o + 1] = unsafe { *p.add(1) };
                target[o + 2] = unsafe { *p };
                target[o + 3] = unsafe { *p.add(3) };
            }
        } else if format == DXGI_FORMAT_R10G10B10A2_UNORM {
            for x in 0..width as usize {
                let value = unsafe { std::ptr::read_unaligned(input.add(x * 4).cast::<u32>()) };
                let o = x * 4;
                target[o] = ((value & 0x3ff) >> 2) as u8;
                target[o + 1] = (((value >> 10) & 0x3ff) >> 2) as u8;
                target[o + 2] = (((value >> 20) & 0x3ff) >> 2) as u8;
                target[o + 3] = 255;
            }
        } else if format == DXGI_FORMAT_R16G16B16A16_FLOAT {
            for x in 0..width as usize {
                let pixel = unsafe { (input.add(x * 8) as *const u16).read_unaligned() };
                let g = unsafe { (input.add(x * 8 + 2) as *const u16).read_unaligned() };
                let b = unsafe { (input.add(x * 8 + 4) as *const u16).read_unaligned() };
                let o = x * 4;
                target[o] = tone_map(f16::from_bits(pixel).to_f32());
                target[o + 1] = tone_map(f16::from_bits(g).to_f32());
                target[o + 2] = tone_map(f16::from_bits(b).to_f32());
                target[o + 3] = 255;
            }
        } else {
            return Err(windows::core::Error::from_thread());
        }
    }
    Ok((output, width, height))
}

fn tone_map(value: f32) -> u8 {
    let mapped = (value.max(0.0) / (1.0 + value.max(0.0))).clamp(0.0, 1.0);
    let srgb = if mapped <= 0.003_130_8 {
        mapped * 12.92
    } else {
        1.055 * mapped.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0).round() as u8
}

pub fn texture_raw(texture: &ID3D11Texture2D) -> *mut c_void {
    Interface::as_raw(texture)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_subimage_rect() {
        let rect = xr::Rect2Di {
            offset: xr::Offset2Di { x: 20, y: -5 },
            extent: xr::Extent2Di {
                width: 100,
                height: 100,
            },
        };
        assert_eq!(resolve_rect(rect, 64, 64), (20, 0, 44, 64));
    }
}
