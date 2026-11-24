//! Two-plane presentation: the console's display shows a title's planes
//! one over the other (GTA VI presents the scene as plane 0 and its UI as
//! plane 1), so the presenter draws plane 0 into the back buffer and blends
//! plane 1 over it, each from its source rectangle to its place on the
//! screen, scaled with a linear filter.
//!
//! The output is 8-bit RGBA: planes in other formats are converted by
//! sampling (HDR values clamp; no tone mapping, and the plane's color space
//! is not applied).
//!
//! Plane 1 is blended as premultiplied alpha (`rgb + dst * (1 - a)`);
//! `VICTORY_PRESENT_ALPHA=straight` blends `rgb * a + dst * (1 - a)`
//! instead, to compare.

use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, ID3DBlob};
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::{PCSTR, PCWSTR, s, w};

use crate::host::{host, transition};

/// A fullscreen triangle whose texture coordinates span the source
/// rectangle over the viewport (the plane's place on the screen).
const SOURCE: &str = r#"
cbuffer Source : register(b0) { float2 origin; float2 extent; };
Texture2D<float4> plane : register(t0);
SamplerState linear_clamp : register(s0);
struct Vertex { float4 position : SV_Position; float2 uv : TEXCOORD0; };
Vertex vs(uint id : SV_VertexID) {
    float2 corner = float2((id << 1) & 2, id & 2);
    Vertex v;
    v.position = float4(corner.x * 2 - 1, 1 - corner.y * 2, 0, 1);
    v.uv = origin + corner * extent;
    return v;
}
float4 ps(Vertex v) : SV_Target { return plane.SampleLevel(linear_clamp, v.uv, 0); }
"#;

/// The back buffer format composition draws into.
pub(crate) const OUTPUT: DXGI_FORMAT = DXGI_FORMAT_R8G8B8A8_UNORM;

/// One plane to draw.
pub(crate) struct Layer<'a> {
    pub(crate) texture: &'a ID3D12Resource,
    /// The host state the texture is in, and is left in.
    pub(crate) state: u32,
    /// The format to sample it as.
    pub(crate) format: DXGI_FORMAT,
    /// The part shown, in texels: left, top, right, bottom.
    pub(crate) source: [u32; 4],
    /// Where it is shown, as fractions of the screen: left, top, right,
    /// bottom.
    pub(crate) destination: [f32; 4],
}

/// The format to sample a plane in, given the title's format for it: its
/// linear twin for sRGB (the bytes as they are, as a copy would show them),
/// a typed member for a typeless family.
pub(crate) fn sampled_format(format: u32) -> DXGI_FORMAT {
    let format = crate::resource::SRGB
        .iter()
        .find(|&&(srgb, _)| srgb == format)
        .map_or(format, |&(_, linear)| linear);
    DXGI_FORMAT(match format {
        1 => 2,   // R32G32B32A32
        9 => 10,  // R16G16B16A16
        23 => 24, // R10G10B10A2
        27 => 28, // R8G8B8A8
        90 => 87, // B8G8R8A8
        f => f,
    } as i32)
}

struct Pipelines {
    root: ID3D12RootSignature,
    /// Plane 0: written as it is (color only).
    opaque: ID3D12PipelineState,
    /// Planes above: blended by their alpha.
    overlay: ID3D12PipelineState,
}

// SAFETY: D3D12 objects are free-threaded.
unsafe impl Send for Pipelines {}
// SAFETY: as above.
unsafe impl Sync for Pipelines {}

fn pipelines() -> Option<&'static Pipelines> {
    static PIPELINES: std::sync::OnceLock<Option<Pipelines>> = std::sync::OnceLock::new();
    PIPELINES
        .get_or_init(|| match make() {
            Ok(p) => Some(p),
            Err(e) => {
                tracing::warn!(error = %e, "no plane composition: plane 0 is shown alone");
                None
            }
        })
        .as_ref()
}

/// Whether composition can run (its shaders compiled and pipelines made).
pub(crate) fn available() -> bool {
    pipelines().is_some()
}

fn make() -> windows::core::Result<Pipelines> {
    let device = &host().ok_or_else(windows::core::Error::empty)?.device;
    let root = root_signature(device)?;
    // Wine's d3dcompiler_47 (vkd3d-shader) may refuse what the system
    // compiler takes on Windows: then DXC, as for the argument clamp.
    let system = fxc(s!("vs"), s!("vs_5_0"))
        .and_then(|vs| Ok((vs, fxc(s!("ps"), s!("ps_5_0"))?)))
        .and_then(|(vs, ps)| build(device, &root, &vs, &ps));
    match system {
        Ok(p) => Ok(p),
        Err(e) => {
            tracing::info!(error = %e, "plane composition: the system compiler failed; trying DXC");
            let vs = dxc(w!("vs"), w!("vs_6_0"))?;
            let ps = dxc(w!("ps"), w!("ps_6_0"))?;
            build(device, &root, &vs, &ps)
        }
    }
}

fn fxc(entry: PCSTR, profile: PCSTR) -> windows::core::Result<Vec<u8>> {
    let mut code: Option<ID3DBlob> = None;
    let mut errors: Option<ID3DBlob> = None;
    // SAFETY: the source and names live through the call; out pointers ours.
    let compiled = unsafe {
        D3DCompile(
            SOURCE.as_ptr().cast(),
            SOURCE.len(),
            s!("compose"),
            None,
            None,
            entry,
            profile,
            0,
            0,
            &mut code,
            Some(&mut errors),
        )
    };
    match (compiled, code) {
        (Ok(()), Some(code)) => Ok(blob_bytes(&code).to_vec()),
        (result, _) => Err(windows::core::Error::new(
            result
                .err()
                .map_or(windows::Win32::Foundation::E_FAIL, |e| e.code()),
            errors.as_ref().map_or(String::new(), |e| {
                String::from_utf8_lossy(blob_bytes(e)).trim().to_owned()
            }),
        )),
    }
}

fn dxc(entry: PCWSTR, profile: PCWSTR) -> windows::core::Result<Vec<u8>> {
    crate::indirect::dxc_compile(SOURCE, profile, entry)
        .inspect_err(|e| tracing::warn!(error = %e, "plane composition: DXC failed too"))
}

fn blob_bytes(blob: &ID3DBlob) -> &[u8] {
    // SAFETY: the blob's own bytes, alive as long as the blob.
    unsafe {
        std::slice::from_raw_parts(blob.GetBufferPointer().cast::<u8>(), blob.GetBufferSize())
    }
}

/// Four constants (the source rectangle's origin and extent in texture
/// coordinates) for the vertex shader, one texture for the pixel shader and
/// a linear clamping sampler.
fn root_signature(device: &ID3D12Device) -> windows::core::Result<ID3D12RootSignature> {
    let range = D3D12_DESCRIPTOR_RANGE {
        RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
        NumDescriptors: 1,
        BaseShaderRegister: 0,
        RegisterSpace: 0,
        OffsetInDescriptorsFromTableStart: 0,
    };
    let parameters = [
        D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                Constants: D3D12_ROOT_CONSTANTS {
                    ShaderRegister: 0,
                    RegisterSpace: 0,
                    Num32BitValues: 4,
                },
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_VERTEX,
        },
        D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                    NumDescriptorRanges: 1,
                    pDescriptorRanges: &range,
                },
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
        },
    ];
    let sampler = D3D12_STATIC_SAMPLER_DESC {
        Filter: D3D12_FILTER_MIN_MAG_LINEAR_MIP_POINT,
        AddressU: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        AddressV: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        AddressW: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        MipLODBias: 0.0,
        MaxAnisotropy: 1,
        ComparisonFunc: D3D12_COMPARISON_FUNC_NEVER,
        BorderColor: D3D12_STATIC_BORDER_COLOR_OPAQUE_BLACK,
        MinLOD: 0.0,
        MaxLOD: D3D12_FLOAT32_MAX,
        ShaderRegister: 0,
        RegisterSpace: 0,
        ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
    };
    let desc = D3D12_ROOT_SIGNATURE_DESC {
        NumParameters: parameters.len() as u32,
        pParameters: parameters.as_ptr(),
        NumStaticSamplers: 1,
        pStaticSamplers: &sampler,
        Flags: D3D12_ROOT_SIGNATURE_FLAG_NONE,
    };
    let mut blob: Option<ID3DBlob> = None;
    // SAFETY: the description points at live locals; out pointer ours.
    unsafe {
        D3D12SerializeRootSignature(&desc, D3D_ROOT_SIGNATURE_VERSION_1, &mut blob, None)?;
        let blob = blob.ok_or_else(windows::core::Error::empty)?;
        device.CreateRootSignature(0, blob_bytes(&blob))
    }
}

fn build(
    device: &ID3D12Device,
    root: &ID3D12RootSignature,
    vs: &[u8],
    ps: &[u8],
) -> windows::core::Result<Pipelines> {
    let straight = std::env::var_os("VICTORY_PRESENT_ALPHA").is_some_and(|v| v == "straight");
    let color = D3D12_COLOR_WRITE_ENABLE_RED.0
        | D3D12_COLOR_WRITE_ENABLE_GREEN.0
        | D3D12_COLOR_WRITE_ENABLE_BLUE.0;
    let target = |blend: bool| D3D12_RENDER_TARGET_BLEND_DESC {
        BlendEnable: blend.into(),
        LogicOpEnable: false.into(),
        SrcBlend: if straight {
            D3D12_BLEND_SRC_ALPHA
        } else {
            D3D12_BLEND_ONE
        },
        DestBlend: D3D12_BLEND_INV_SRC_ALPHA,
        BlendOp: D3D12_BLEND_OP_ADD,
        SrcBlendAlpha: D3D12_BLEND_ONE,
        DestBlendAlpha: D3D12_BLEND_INV_SRC_ALPHA,
        BlendOpAlpha: D3D12_BLEND_OP_ADD,
        LogicOp: D3D12_LOGIC_OP_NOOP,
        // The back buffer's alpha stays the clear's 1.
        RenderTargetWriteMask: color as u8,
    };
    let pipeline = |blend: bool| {
        let mut targets = [D3D12_RENDER_TARGET_BLEND_DESC::default(); 8];
        targets[0] = target(blend);
        let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
        formats[0] = OUTPUT;
        let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            // SAFETY: a borrowed pointer the description never releases.
            pRootSignature: unsafe { std::mem::transmute_copy(root) },
            VS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: vs.as_ptr().cast(),
                BytecodeLength: vs.len(),
            },
            PS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: ps.as_ptr().cast(),
                BytecodeLength: ps.len(),
            },
            BlendState: D3D12_BLEND_DESC {
                AlphaToCoverageEnable: false.into(),
                IndependentBlendEnable: false.into(),
                RenderTarget: targets,
            },
            SampleMask: u32::MAX,
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_NONE,
                DepthClipEnable: true.into(),
                ..Default::default()
            },
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
                DepthEnable: false.into(),
                DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ZERO,
                DepthFunc: D3D12_COMPARISON_FUNC_ALWAYS,
                ..Default::default()
            },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            RTVFormats: formats,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            ..Default::default()
        };
        // SAFETY: the description points at live data.
        unsafe { device.CreateGraphicsPipelineState::<ID3D12PipelineState>(&desc) }
    };
    let pipelines = Pipelines {
        root: root.clone(),
        opaque: pipeline(false)?,
        overlay: pipeline(true)?,
    };
    tracing::info!(
        alpha = if straight {
            "straight"
        } else {
            "premultiplied"
        },
        "plane composition ready"
    );
    Ok(pipelines)
}

/// A presenter's descriptors for composition: one shader-visible view per
/// plane and the back buffer's render target view. They are rewritten
/// every frame (every submission is waited for, so none is in use then).
pub(crate) struct Composer {
    views: ID3D12DescriptorHeap,
    view_size: u32,
    target: ID3D12DescriptorHeap,
}

/// Planes composed at most.
pub(crate) const LAYERS: usize = 2;

impl Composer {
    pub(crate) fn new(device: &ID3D12Device) -> windows::core::Result<Composer> {
        // SAFETY: plain object creation.
        unsafe {
            let views = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                NumDescriptors: LAYERS as u32,
                Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                NodeMask: 0,
            })?;
            let view_size =
                device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
            let target = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: 1,
                ..Default::default()
            })?;
            Ok(Composer {
                views,
                view_size,
                target,
            })
        }
    }

    /// Records, in `list`, `layers` (at most [`LAYERS`], bottom first)
    /// drawn into `back`, a `width` by `height` [`OUTPUT`] texture in the
    /// render target state, after clearing it to black: the first written
    /// as it is, those above blended. Each texture is moved to the pixel
    /// shader resource state for the draws and back after.
    pub(crate) fn record(
        &self,
        device: &ID3D12Device,
        list: &ID3D12GraphicsCommandList,
        back: &ID3D12Resource,
        (width, height): (u32, u32),
        layers: &[Layer],
    ) {
        let Some(p) = pipelines() else { return };
        let sampled = D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
        let moves = |to_sampled: bool| -> Vec<D3D12_RESOURCE_BARRIER> {
            layers
                .iter()
                .filter(|l| l.state != sampled.0 as u32)
                .map(|l| {
                    let own = D3D12_RESOURCE_STATES(l.state as i32);
                    if to_sampled {
                        transition(l.texture, own, sampled)
                    } else {
                        transition(l.texture, sampled, own)
                    }
                })
                .collect()
        };
        // SAFETY: an open list, our heaps and pipelines, live textures.
        unsafe {
            let to = moves(true);
            if !to.is_empty() {
                list.ResourceBarrier(&to);
            }
            let rtv = self.target.GetCPUDescriptorHandleForHeapStart();
            device.CreateRenderTargetView(back, None, rtv);
            let cpu = self.views.GetCPUDescriptorHandleForHeapStart();
            let gpu = self.views.GetGPUDescriptorHandleForHeapStart();
            for (i, l) in layers.iter().take(LAYERS).enumerate() {
                let view = D3D12_SHADER_RESOURCE_VIEW_DESC {
                    Format: l.format,
                    ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
                    Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                    Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                        Texture2D: D3D12_TEX2D_SRV {
                            MostDetailedMip: 0,
                            MipLevels: 1,
                            PlaneSlice: 0,
                            ResourceMinLODClamp: 0.0,
                        },
                    },
                };
                device.CreateShaderResourceView(
                    l.texture,
                    Some(&view),
                    D3D12_CPU_DESCRIPTOR_HANDLE {
                        ptr: cpu.ptr + i * self.view_size as usize,
                    },
                );
            }
            list.ClearRenderTargetView(rtv, &[0.0, 0.0, 0.0, 1.0], None);
            list.OMSetRenderTargets(1, Some(&rtv), false, None);
            list.SetDescriptorHeaps(&[Some(self.views.clone())]);
            list.SetGraphicsRootSignature(&p.root);
            list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            for (i, l) in layers.iter().take(LAYERS).enumerate() {
                let desc = l.texture.GetDesc();
                let (tw, th) = (desc.Width as f32, desc.Height as f32);
                let [left, top, right, bottom] = l.source.map(|v| v as f32);
                let constants = [
                    left / tw,
                    top / th,
                    (right - left) / tw,
                    (bottom - top) / th,
                ];
                let [dl, dt, dr, db] = l.destination;
                let (w, h) = (width as f32, height as f32);
                list.RSSetViewports(&[D3D12_VIEWPORT {
                    TopLeftX: dl * w,
                    TopLeftY: dt * h,
                    Width: (dr - dl) * w,
                    Height: (db - dt) * h,
                    MinDepth: 0.0,
                    MaxDepth: 1.0,
                }]);
                list.RSSetScissorRects(&[windows::Win32::Foundation::RECT {
                    left: (dl * w).round() as i32,
                    top: (dt * h).round() as i32,
                    right: (dr * w).round() as i32,
                    bottom: (db * h).round() as i32,
                }]);
                list.SetPipelineState(if i == 0 { &p.opaque } else { &p.overlay });
                list.SetGraphicsRoot32BitConstants(0, 4, constants.as_ptr().cast(), 0);
                list.SetGraphicsRootDescriptorTable(
                    1,
                    D3D12_GPU_DESCRIPTOR_HANDLE {
                        ptr: gpu.ptr + (i * self.view_size as usize) as u64,
                    },
                );
                list.DrawInstanced(3, 1, 0, 0);
            }
            let back_to = moves(false);
            if !back_to.is_empty() {
                list.ResourceBarrier(&back_to);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The composition shaders compile and the host takes the pipelines.
    #[test]
    fn plane_composition_builds() {
        if host().is_none() {
            return; // no host GPU here
        }
        assert!(available());
    }

    /// An 8x4 render target and its view.
    fn target(device: &ID3D12Device) -> (ID3D12Resource, ID3D12DescriptorHeap) {
        // SAFETY: plain object creation.
        unsafe {
            let mut texture: Option<ID3D12Resource> = None;
            device
                .CreateCommittedResource(
                    &D3D12_HEAP_PROPERTIES {
                        Type: D3D12_HEAP_TYPE_DEFAULT,
                        ..Default::default()
                    },
                    D3D12_HEAP_FLAG_NONE,
                    &D3D12_RESOURCE_DESC {
                        Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                        Width: 8,
                        Height: 4,
                        DepthOrArraySize: 1,
                        MipLevels: 1,
                        Format: OUTPUT,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
                        ..Default::default()
                    },
                    D3D12_RESOURCE_STATE_RENDER_TARGET,
                    None,
                    &mut texture,
                )
                .unwrap();
            let texture = texture.unwrap();
            let heap: ID3D12DescriptorHeap = device
                .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                    Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                    NumDescriptors: 1,
                    ..Default::default()
                })
                .unwrap();
            device.CreateRenderTargetView(
                &texture,
                None,
                heap.GetCPUDescriptorHandleForHeapStart(),
            );
            (texture, heap)
        }
    }

    /// Plane 1 (premultiplied half-transparent red) over plane 0 (blue) on
    /// the left half of the screen; the right half shows plane 0 alone.
    #[test]
    fn plane_one_is_blended_over_plane_zero_at_its_placement() {
        let Some(host) = host() else {
            return; // no host GPU here
        };
        if std::env::var_os("VICTORY_PRESENT_ALPHA").is_some() {
            return; // the expected colors are premultiplied blending's
        }
        let device = &host.device;
        let colors = [[0.0, 0.0, 1.0, 1.0], [0.5, 0.0, 0.0, 0.5]];
        let planes = [target(device), target(device)];
        let (back, _) = target(device);
        let composer = Composer::new(device).unwrap();
        let list = host.new_list().unwrap();
        let l = &list.list;
        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        // SAFETY: an open list, live textures and a readback buffer of the
        // footprint's size, mapped after the list ran.
        unsafe {
            for ((_, heap), color) in planes.iter().zip(&colors) {
                l.ClearRenderTargetView(heap.GetCPUDescriptorHandleForHeapStart(), color, None);
            }
            let layers = [
                Layer {
                    texture: &planes[0].0,
                    state: D3D12_RESOURCE_STATE_RENDER_TARGET.0 as u32,
                    format: OUTPUT,
                    source: [0, 0, 8, 4],
                    destination: [0.0, 0.0, 1.0, 1.0],
                },
                Layer {
                    texture: &planes[1].0,
                    state: D3D12_RESOURCE_STATE_RENDER_TARGET.0 as u32,
                    format: OUTPUT,
                    source: [0, 0, 8, 4],
                    destination: [0.0, 0.0, 0.5, 1.0],
                },
            ];
            composer.record(device, l, &back, (8, 4), &layers);
            let mut total = 0;
            device.GetCopyableFootprints(
                &back.GetDesc(),
                0,
                1,
                0,
                Some(&mut footprint),
                None,
                None,
                Some(&mut total),
            );
            let mut buffer: Option<ID3D12Resource> = None;
            device
                .CreateCommittedResource(
                    &D3D12_HEAP_PROPERTIES {
                        Type: D3D12_HEAP_TYPE_READBACK,
                        ..Default::default()
                    },
                    D3D12_HEAP_FLAG_NONE,
                    &D3D12_RESOURCE_DESC {
                        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                        Width: total,
                        Height: 1,
                        DepthOrArraySize: 1,
                        MipLevels: 1,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                        ..Default::default()
                    },
                    D3D12_RESOURCE_STATE_COPY_DEST,
                    None,
                    &mut buffer,
                )
                .unwrap();
            let buffer = buffer.unwrap();
            l.ResourceBarrier(&[transition(
                &back,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            )]);
            l.CopyTextureRegion(
                &D3D12_TEXTURE_COPY_LOCATION {
                    pResource: std::mem::transmute_copy(&buffer),
                    Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                        PlacedFootprint: footprint,
                    },
                },
                0,
                0,
                0,
                &D3D12_TEXTURE_COPY_LOCATION {
                    pResource: std::mem::transmute_copy(&back),
                    Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                        SubresourceIndex: 0,
                    },
                },
                None,
            );
            list.close();
            host.execute(&[&list]);
            let mut data = std::ptr::null_mut();
            buffer.Map(0, None, Some(&mut data)).unwrap();
            let pitch = footprint.Footprint.RowPitch as usize;
            let rows = std::slice::from_raw_parts(data.cast::<u8>(), pitch * 4);
            let pixel =
                |x: usize, y: usize| <[u8; 4]>::try_from(&rows[y * pitch + x * 4..][..4]).unwrap();
            let near = |a: [u8; 4], b: [u8; 4]| a.iter().zip(b).all(|(&a, b)| a.abs_diff(b) <= 1);
            for y in 0..4 {
                for x in 0..8 {
                    let want = if x < 4 {
                        [128, 0, 128, 255]
                    } else {
                        [0, 0, 255, 255]
                    };
                    assert!(
                        near(pixel(x, y), want),
                        "({x}, {y}): {:?}, not {want:?}",
                        pixel(x, y)
                    );
                }
            }
            buffer.Unmap(0, None);
        }
    }

    #[test]
    fn planes_are_sampled_as_their_bytes() {
        assert_eq!(sampled_format(29), DXGI_FORMAT_R8G8B8A8_UNORM);
        assert_eq!(sampled_format(27), DXGI_FORMAT_R8G8B8A8_UNORM);
        assert_eq!(sampled_format(91), DXGI_FORMAT_B8G8R8A8_UNORM);
        assert_eq!(sampled_format(23), DXGI_FORMAT_R10G10B10A2_UNORM);
        assert_eq!(sampled_format(26), DXGI_FORMAT_R11G11B10_FLOAT);
    }
}
