//! The wgpu side of Appolis.
//!
//! One offscreen pass draws sky, ground and every building as one instanced
//! draw. Egui's own pass has no depth buffer and no place for extra targets, so
//! the result is composited into the callback viewport as a textured quad. That
//! also gives the HDR/bloom pass somewhere to live later.

use std::sync::Arc;

use egui_wgpu::wgpu;
use wgpu::util::DeviceExt;

use crate::scene::{Building, Mood, MAX_BUILDINGS};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const CITY_WGSL: &str = include_str!("shaders/city.wgsl");
const COMPOSITE_WGSL: &str = include_str!("shaders/composite.wgsl");
const INSTANCE_CAPACITY: usize = MAX_BUILDINGS;
const MAX_TARGET_SIDE: u32 = 4096;
/// WebGPU requires 4x support for every renderable format, so this needs no
/// adapter query. Geometry edges resolve once, then the composite samples the
/// single-sample result.
const SAMPLES: u32 = 4;
/// 20 cars, 8 drones, 18 monorail segments, 6 pillars, 6 train cars, 9
/// airship boxes, 160 pedestrians, 16 billboard boxes, 32 trees and lamps,
/// and 6 wisps. Purely decorative; the shader places them.
const TRAFFIC_COUNT: u32 = 305;
/// Camera shared by the renderer and the pointer picker, so a hover always
/// lands on the tower that is actually under the cursor. Raised well above
/// the roofs for the toy-diorama look.
const EYE_BASE: [f32; 3] = [26.0, 72.0, 40.0];
const EYE_TARGET: [f32; 3] = [0.0, 0.0, 0.0];
const FOV_Y_DEG: f32 = 34.0;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [f32; 16],
    params: [f32; 4],
    /// x: local day fraction 0..1, y: highlighted instance (-1 none),
    /// z: highlight strength, w: spare.
    world: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuBuilding {
    center: [f32; 3],
    half: [f32; 3],
    body: [f32; 3],
    glow: [f32; 3],
    seed: f32,
    _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CubeVertex {
    pos: [f32; 3],
    normal: [f32; 3],
}

impl CubeVertex {
    const fn new(pos: [f32; 3], normal: [f32; 3]) -> Self {
        Self { pos, normal }
    }
}

/// Six faces, four corners each, wound so both sides can be drawn without
/// worrying about which way a box faces.
const CUBE: [CubeVertex; 24] = [
    // +X
    CubeVertex::new([0.5, -0.5, -0.5], [1.0, 0.0, 0.0]),
    CubeVertex::new([0.5, -0.5, 0.5], [1.0, 0.0, 0.0]),
    CubeVertex::new([0.5, 0.5, 0.5], [1.0, 0.0, 0.0]),
    CubeVertex::new([0.5, 0.5, -0.5], [1.0, 0.0, 0.0]),
    // -X
    CubeVertex::new([-0.5, -0.5, 0.5], [-1.0, 0.0, 0.0]),
    CubeVertex::new([-0.5, -0.5, -0.5], [-1.0, 0.0, 0.0]),
    CubeVertex::new([-0.5, 0.5, -0.5], [-1.0, 0.0, 0.0]),
    CubeVertex::new([-0.5, 0.5, 0.5], [-1.0, 0.0, 0.0]),
    // +Y
    CubeVertex::new([-0.5, 0.5, -0.5], [0.0, 1.0, 0.0]),
    CubeVertex::new([0.5, 0.5, -0.5], [0.0, 1.0, 0.0]),
    CubeVertex::new([0.5, 0.5, 0.5], [0.0, 1.0, 0.0]),
    CubeVertex::new([-0.5, 0.5, 0.5], [0.0, 1.0, 0.0]),
    // -Y
    CubeVertex::new([-0.5, -0.5, 0.5], [0.0, -1.0, 0.0]),
    CubeVertex::new([0.5, -0.5, 0.5], [0.0, -1.0, 0.0]),
    CubeVertex::new([0.5, -0.5, -0.5], [0.0, -1.0, 0.0]),
    CubeVertex::new([-0.5, -0.5, -0.5], [0.0, -1.0, 0.0]),
    // +Z
    CubeVertex::new([-0.5, -0.5, 0.5], [0.0, 0.0, 1.0]),
    CubeVertex::new([0.5, -0.5, 0.5], [0.0, 0.0, 1.0]),
    CubeVertex::new([0.5, 0.5, 0.5], [0.0, 0.0, 1.0]),
    CubeVertex::new([-0.5, 0.5, 0.5], [0.0, 0.0, 1.0]),
    // -Z
    CubeVertex::new([0.5, -0.5, -0.5], [0.0, 0.0, -1.0]),
    CubeVertex::new([-0.5, -0.5, -0.5], [0.0, 0.0, -1.0]),
    CubeVertex::new([-0.5, 0.5, -0.5], [0.0, 0.0, -1.0]),
    CubeVertex::new([0.5, 0.5, -0.5], [0.0, 0.0, -1.0]),
];

const CUBE_INDICES: [u16; 36] = [
    0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7, 8, 9, 10, 8, 10, 11, 12, 13, 14, 12, 14, 15, 16, 17, 18,
    16, 18, 19, 20, 21, 22, 20, 22, 23,
];

const GROUND: [[f32; 2]; 6] = [
    [-120.0, -120.0],
    [120.0, -120.0],
    [120.0, 120.0],
    [-120.0, -120.0],
    [120.0, 120.0],
    [-120.0, 120.0],
];

struct Targets {
    /// Single-sample resolve, kept so an export can copy it to a readback
    /// buffer and the composite can sample it.
    color_texture: wgpu::Texture,
    color: wgpu::TextureView,
    /// Multisampled target the scene actually draws into.
    msaa: wgpu::TextureView,
    depth: wgpu::TextureView,
    /// Linear, so supersampled frames resolve smoothly.
    bind: wgpu::BindGroup,
    size: [u32; 2],
}

pub struct CityRenderer {
    globals: wgpu::Buffer,
    instances: wgpu::Buffer,
    cube_vertices: wgpu::Buffer,
    cube_indices: wgpu::Buffer,
    ground_vertices: wgpu::Buffer,
    bind: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    sky: wgpu::RenderPipeline,
    ground: wgpu::RenderPipeline,
    buildings: wgpu::RenderPipeline,
    traffic: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    composite_layout: wgpu::BindGroupLayout,
    targets: Option<Targets>,
    target_format: wgpu::TextureFormat,
    /// Wall-clock seconds of the last offscreen render, for the 30 fps cap.
    last_render: f64,
}

impl CityRenderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("appolis_city"),
            source: wgpu::ShaderSource::Wgsl(CITY_WGSL.into()),
        });
        let composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("appolis_composite"),
            source: wgpu::ShaderSource::Wgsl(COMPOSITE_WGSL.into()),
        });

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appolis_globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appolis_buildings"),
            size: (INSTANCE_CAPACITY * std::mem::size_of::<GpuBuilding>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let cube_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("appolis_cube_vertices"),
            contents: bytemuck::cast_slice(&CUBE),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let cube_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("appolis_cube_indices"),
            contents: bytemuck::cast_slice(&CUBE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let ground_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("appolis_ground"),
            contents: bytemuck::cast_slice(&GROUND),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("appolis_scene_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("appolis_scene_bind"),
            layout: &scene_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });

        let scene_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("appolis_scene_pipeline_layout"),
                bind_group_layouts: &[Some(&scene_layout)],
                immediate_size: 0,
            });

        let cube_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CubeVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 12,
                    shader_location: 1,
                },
            ],
        };
        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuBuilding>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 12,
                    shader_location: 3,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 24,
                    shader_location: 4,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 36,
                    shader_location: 5,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 48,
                    shader_location: 6,
                },
            ],
        };
        let ground_layout = wgpu::VertexBufferLayout {
            array_stride: 8,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };

        let no_depth = wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        };
        let depth = wgpu::DepthStencilState {
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            ..no_depth.clone()
        };

        let sky = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("appolis_sky"),
            layout: Some(&scene_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_sky"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(no_depth.clone()),
            multisample: wgpu::MultisampleState {
                count: SAMPLES,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_sky"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let ground = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("appolis_ground"),
            layout: Some(&scene_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_ground"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(ground_layout.clone())],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(depth.clone()),
            multisample: wgpu::MultisampleState {
                count: SAMPLES,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_ground"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let buildings = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("appolis_buildings"),
            layout: Some(&scene_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_building"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(cube_layout.clone()), Some(instance_layout)],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                // Boxes are closed; drawing both faces avoids depending on
                // winding at a hundred fragments each.
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth.clone()),
            multisample: wgpu::MultisampleState {
                count: SAMPLES,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_building"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let traffic = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("appolis_traffic"),
            layout: Some(&scene_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_traffic"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(cube_layout)],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth.clone()),
            multisample: wgpu::MultisampleState {
                count: SAMPLES,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_traffic"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let composite_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("appolis_composite_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let composite_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("appolis_composite_pipeline_layout"),
                bind_group_layouts: &[Some(&composite_layout)],
                immediate_size: 0,
            });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("appolis_sampler_linear"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let composite = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("appolis_composite"),
            layout: Some(&composite_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &composite_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &composite_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Self {
            globals,
            instances,
            cube_vertices,
            cube_indices,
            ground_vertices,
            bind,
            sampler,
            sky,
            ground,
            buildings,
            traffic,
            composite,
            composite_layout,
            targets: None,
            target_format,
            last_render: f64::NEG_INFINITY,
        }
    }

    fn ensure_targets(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return;
        }

        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("appolis_color"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("appolis_depth"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: SAMPLES,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let msaa = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("appolis_msaa"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: SAMPLES,
            dimension: wgpu::TextureDimension::D2,
            format: self.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
        let msaa_view = msaa.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        let make_bind = |sampler: &wgpu::Sampler| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("appolis_composite_bind"),
                layout: &self.composite_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&color_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            })
        };
        let bind = make_bind(&self.sampler);

        self.targets = Some(Targets {
            color_texture: color,
            color: color_view,
            msaa: msaa_view,
            depth: depth_view,
            bind,
            size,
        });
    }

    fn record(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        count: u32,
        traffic: bool,
    ) {
        let Some(targets) = self.targets.as_ref() else {
            return;
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("appolis_offscreen"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &targets.msaa,
                depth_slice: None,
                resolve_target: Some(&targets.color),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.42,
                        g: 0.62,
                        b: 0.86,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.sky);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.draw(0..3, 0..1);

        pass.set_pipeline(&self.ground);
        pass.set_vertex_buffer(0, self.ground_vertices.slice(..));
        pass.draw(0..GROUND.len() as u32, 0..1);

        if count > 0 {
            pass.set_pipeline(&self.buildings);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_vertex_buffer(0, self.cube_vertices.slice(..));
            pass.set_vertex_buffer(1, self.instances.slice(..));
            pass.set_index_buffer(self.cube_indices.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..CUBE_INDICES.len() as u32, 0, 0..count);
        }

        // Hover cars and drones live entirely in the vertex shader: their
        // positions come from the instance index and the clock, so there is
        // no per-frame buffer to update.
        if traffic {
            pass.set_pipeline(&self.traffic);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_vertex_buffer(0, self.cube_vertices.slice(..));
            pass.set_index_buffer(self.cube_indices.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..CUBE_INDICES.len() as u32, 0, 0..TRAFFIC_COUNT);
        }
    }

    /// Write the shared uniforms and building instances for one frame.
    fn write_frame(
        &self,
        queue: &wgpu::Queue,
        time: f32,
        look: &CityLook,
        buildings: &[Building],
        aspect: f32,
        fov_deg: f32,
    ) -> u32 {
        let (eye, _, _, _) = camera_basis(time);
        let view = look_at(eye, EYE_TARGET, [0.0, 1.0, 0.0]);
        let projection = perspective(fov_deg.to_radians(), aspect, 0.1, 320.0);
        let view_proj = multiply(projection, view);

        let intensity = look.intensity.clamp(0.0, 1.0);
        queue.write_buffer(
            &self.globals,
            0,
            bytemuck::bytes_of(&Globals {
                view_proj: flatten(view_proj),
                params: [
                    time % 3600.0,
                    look.mood.code(),
                    intensity,
                    0.3 + 0.22 * intensity,
                ],
                world: [
                    look.hour.rem_euclid(1.0),
                    look.highlight.map_or(-1.0, |index| index as f32),
                    1.0,
                    if look.blueprint { 1.0 } else { 0.0 },
                ],
            }),
        );

        let count = buildings.len().min(INSTANCE_CAPACITY);
        let gpu: Vec<GpuBuilding> = buildings
            .iter()
            .take(count)
            .map(|b| GpuBuilding {
                center: b.center,
                half: b.half,
                body: b.body,
                glow: b.glow,
                seed: b.seed,
                _pad: 0.0,
            })
            .collect();
        if !gpu.is_empty() {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&gpu));
        }
        count as u32
    }

    /// Render one square frame and read it back as straight RGBA. Used only for
    /// the share export; the temporary target is replaced by the panel-sized
    /// one on the next live frame.
    pub fn export_rgba(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: u32,
        time: f32,
        look: &CityLook,
        buildings: &[Building],
    ) -> Result<(Vec<u8>, u32, u32), String> {
        let n = size.clamp(256, MAX_TARGET_SIDE / 2);
        // Render at 2x and average down: a one-shot image can afford the
        // cleanest edges the share card will ever get.
        let render = n * 2;
        self.ensure_targets(device, [render, render]);
        let count = self.write_frame(queue, time, look, buildings, 1.0, 60.0);
        let target = self.targets.as_ref().ok_or("no render target")?;
        let stride = wgpu::util::align_to(render * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appolis_export_readback"),
            size: (stride * render) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("appolis_export"),
        });
        self.record(&mut encoder, count, look.traffic);
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.color_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(render),
                },
            },
            wgpu::Extent3d {
                width: render,
                height: render,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);

        let (tx, rx) = std::sync::mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, 0..buffer.size(), move |result| {
            let _ = tx.send(result);
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| format!("GPU poll failed: {e:?}"))?;
        loop {
            match rx.try_recv() {
                Ok(Ok(())) => break,
                Ok(Err(e)) => return Err(format!("GPU readback failed: {e}")),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return Err("GPU readback was dropped".to_string())
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    let _ = device.poll(wgpu::PollType::Poll);
                    std::thread::yield_now();
                }
            }
        }
        let view = buffer
            .get_mapped_range(0..buffer.size())
            .map_err(|e| format!("buffer map failed: {e:?}"))?;
        let stride = stride as usize;
        let mut rgba = Vec::with_capacity((n * n * 4) as usize);
        for y in 0..n as usize {
            let top = (y * 2) * stride;
            let bottom = (y * 2 + 1) * stride;
            for x in 0..n as usize {
                let left = x * 2 * 4;
                let right = left + 4;
                for channel in 0..4 {
                    let sum = view[top + left + channel] as u32
                        + view[top + right + channel] as u32
                        + view[bottom + left + channel] as u32
                        + view[bottom + right + channel] as u32;
                    rgba.push((sum / 4) as u8);
                }
            }
        }
        drop(view);
        buffer.unmap();
        // The surface format is chosen by the adapter; PNG wants RGBA.
        if matches!(
            self.target_format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            for pixel in rgba.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
        }
        Ok((rgba, n, n))
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'static>) {
        if let Some(targets) = self.targets.as_ref() {
            pass.set_pipeline(&self.composite);
            pass.set_bind_group(0, &targets.bind, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

/// What the city should look like this frame. Kept as one value so the
/// preview call does not grow an argument per effect.
#[derive(Clone, Debug)]
pub struct CityLook {
    pub mood: Mood,
    pub intensity: f32,
    /// Local time of day, 0.0 at midnight to 1.0 at the next midnight.
    pub hour: f32,
    /// Building under the pointer, or pinned by a click.
    pub highlight: Option<usize>,
    /// Offscreen render scale; above 1.0 supersamples, and the composite
    /// stretches it back to the panel.
    pub render_scale: f32,
    /// Cars and drones; off in the low preset.
    pub traffic: bool,
    /// Cap for the offscreen pass, matching the preset.
    pub fps: u32,
    /// Cyan blueprint schematic instead of the lit city.
    pub blueprint: bool,
}

impl Default for CityLook {
    fn default() -> Self {
        Self {
            mood: Mood::Idle,
            intensity: 0.0,
            hour: 0.0,
            highlight: None,
            render_scale: 1.0,
            traffic: true,
            fps: 30,
            blueprint: false,
        }
    }
}

/// One frame of the city, drawn inside an egui paint callback.
pub struct CityPreview {
    time: f32,
    size_px: [u32; 2],
    look: CityLook,
    buildings: Arc<[Building]>,
}

impl CityPreview {
    pub fn new(time: f32, size_px: [u32; 2], look: CityLook, buildings: Arc<[Building]>) -> Self {
        Self {
            time,
            size_px,
            look,
            buildings,
        }
    }
}

impl egui_wgpu::CallbackTrait for CityPreview {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(renderer) = callback_resources.get_mut::<CityRenderer>() else {
            return Vec::new();
        };

        let size = [
            self.size_px[0].clamp(1, MAX_TARGET_SIDE),
            self.size_px[1].clamp(1, MAX_TARGET_SIDE),
        ];
        let scale = if self.look.render_scale.is_finite() {
            self.look.render_scale.clamp(0.35, 1.5)
        } else {
            1.0
        };
        let render_size = [
            ((size[0] as f32 * scale).round() as u32).clamp(1, MAX_TARGET_SIDE),
            ((size[1] as f32 * scale).round() as u32).clamp(1, MAX_TARGET_SIDE),
        ];
        // Egui may paint faster than the display needs; the city is capped and
        // egui composites the previous frame in between.
        let fresh = renderer
            .targets
            .as_ref()
            .is_none_or(|t| t.size != render_size);
        if !fresh && self.time as f64 - renderer.last_render < 1.0 / self.look.fps.max(1) as f64 {
            return Vec::new();
        }
        renderer.last_render = self.time as f64;
        renderer.ensure_targets(device, render_size);

        let aspect = render_size[0] as f32 / render_size[1] as f32;
        let count = renderer.write_frame(
            queue,
            self.time,
            &self.look,
            &self.buildings,
            aspect,
            FOV_Y_DEG,
        );

        renderer.record(egui_encoder, count, self.look.traffic);
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::epaint::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(renderer) = callback_resources.get::<CityRenderer>() {
            renderer.paint(render_pass);
        }
    }
}

type Mat4 = [[f32; 4]; 4];

/// Camera eye and basis for a moment, shared by rendering and picking.
fn camera_basis(time: f32) -> ([f32; 3], [f32; 3], [f32; 3], [f32; 3]) {
    let eye = [
        EYE_BASE[0] + (time * 0.03).sin() * 2.0,
        EYE_BASE[1],
        EYE_BASE[2],
    ];
    let forward = normalize(sub(EYE_TARGET, eye));
    let right = normalize(cross(forward, [0.0, 1.0, 0.0]));
    let up = cross(right, forward);
    (eye, forward, right, up)
}

/// The tower under a pointer position, in physical pixels from the top-left of
/// the city panel. A ray/AABB test picks the nearest box, so a tall tower in
/// front wins over one behind it.
pub fn pick(
    buildings: &[Building],
    size_px: [u32; 2],
    time: f32,
    pointer_px: [f32; 2],
) -> Option<usize> {
    if size_px[0] == 0 || size_px[1] == 0 {
        return None;
    }
    let (eye, forward, right, up) = camera_basis(time);
    let ndc_x = pointer_px[0] / size_px[0] as f32 * 2.0 - 1.0;
    let ndc_y = 1.0 - pointer_px[1] / size_px[1] as f32 * 2.0;
    let tan_half = (FOV_Y_DEG * 0.5).to_radians().tan();
    let aspect = size_px[0] as f32 / size_px[1] as f32;
    let dir = normalize(add(
        forward,
        add(
            scale(right, ndc_x * tan_half * aspect),
            scale(up, ndc_y * tan_half),
        ),
    ));

    let mut best: Option<(f32, usize)> = None;
    for (index, b) in buildings.iter().enumerate() {
        // Scenery is not interactive: hovering empty ground or a filler block
        // must not open a blank card.
        if b.app.is_empty() {
            continue;
        }
        if let Some(t) = ray_box(eye, dir, sub(b.center, b.half), add(b.center, b.half)) {
            if best.is_none_or(|(nearest, _)| t < nearest) {
                best = Some((t, index));
            }
        }
    }
    best.map(|(_, index)| index)
}

fn ray_box(
    origin: [f32; 3],
    dir: [f32; 3],
    min: [f32; 3],
    max: [f32; 3],
) -> Option<f32> {
    let mut near = 0.0f32;
    let mut far = f32::INFINITY;
    for axis in 0..3 {
        if dir[axis].abs() < 1e-6 {
            if origin[axis] < min[axis] || origin[axis] > max[axis] {
                return None;
            }
            continue;
        }
        let inv = 1.0 / dir[axis];
        let mut t0 = (min[axis] - origin[axis]) * inv;
        let mut t1 = (max[axis] - origin[axis]) * inv;
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }
        near = near.max(t0);
        far = far.min(t1);
        if near > far {
            return None;
        }
    }
    (far > 0.0).then_some(near)
}

fn multiply(a: Mat4, b: Mat4) -> Mat4 {
    let mut out = [[0.0f32; 4]; 4];
    for (c, col) in out.iter_mut().enumerate() {
        for (r, value) in col.iter_mut().enumerate() {
            *value = (0..4).map(|k| a[k][r] * b[c][k]).sum();
        }
    }
    out
}

fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let f = 1.0 / (fov_y * 0.5).tan();
    let mut m = [[0.0f32; 4]; 4];
    m[0][0] = f / aspect;
    m[1][1] = f;
    m[2][2] = far / (near - far);
    m[2][3] = -1.0;
    m[3][2] = (far * near) / (near - far);
    m
}

fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> Mat4 {
    let f = normalize(sub(target, eye));
    let s = normalize(cross(f, up));
    let u = cross(s, f);
    [
        [s[0], u[0], -f[0], 0.0],
        [s[1], u[1], -f[1], 0.0],
        [s[2], u[2], -f[2], 0.0],
        [
            -dot(s, eye),
            -dot(u, eye),
            dot(f, eye),
            1.0,
        ],
    ]
}

fn flatten(m: Mat4) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for c in 0..4 {
        for r in 0..4 {
            out[c * 4 + r] = m[c][r];
        }
    }
    out
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = dot(v, v).sqrt().max(1e-6);
    [v[0] / len, v[1] / len, v[2] / len]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{layout, CityApp, CitySnapshot, District};

    #[test]
    fn camera_matrix_is_finite_and_maps_the_city_in_front() {
        let view = look_at(EYE_BASE, EYE_TARGET, [0.0, 1.0, 0.0]);
        let proj = perspective(FOV_Y_DEG.to_radians(), 2.3, 0.1, 320.0);
        let vp = multiply(proj, view);
        for v in flatten(vp) {
            assert!(v.is_finite());
        }

        // The core tower base should land in front of the camera (w > 0).
        let p = [
            vp[0][0] * 0.0 + vp[1][0] * 0.0 + vp[2][0] * 0.0 + vp[3][0],
            vp[0][1] * 0.0 + vp[1][1] * 0.0 + vp[2][1] * 0.0 + vp[3][1],
            vp[0][2] * 0.0 + vp[1][2] * 0.0 + vp[2][2] * 0.0 + vp[3][2],
            vp[0][3] * 0.0 + vp[1][3] * 0.0 + vp[2][3] * 0.0 + vp[3][3],
        ];
        assert!(p[3] > 0.0, "core must be in front of the camera");
        assert!(p[0] / p[3] > -1.0 && p[0] / p[3] < 1.0);
    }

    #[test]
    fn shaders_parse_and_validate() {
        for (name, source) in [("city", CITY_WGSL), ("composite", COMPOSITE_WGSL)] {
            let module = naga::front::wgsl::parse_str(source)
                .unwrap_or_else(|e| panic!("{name}.wgsl: {}", e.emit_to_string(source)));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap_or_else(|e| panic!("{name}.wgsl: {e:?}"));
        }
    }

    fn one_app_city() -> Vec<Building> {
        layout(&CitySnapshot {
            apps: vec![CityApp {
                name: "Code.exe".to_string(),
                district: District::Coding,
                minutes: 60,
                score: 82.0,
                switches: 4,
                active: true,
            }],
            mood: Mood::Deep,
            score: 82.0,
        })
    }

    fn project(point: [f32; 3], size: [u32; 2], time: f32) -> [f32; 2] {
        let (eye, _, _, _) = camera_basis(time);
        let view = look_at(eye, EYE_TARGET, [0.0, 1.0, 0.0]);
        let proj = perspective(
            FOV_Y_DEG.to_radians(),
            size[0] as f32 / size[1] as f32,
            0.1,
            320.0,
        );
        let vp = multiply(proj, view);
        let clip = [
            vp[0][0] * point[0] + vp[1][0] * point[1] + vp[2][0] * point[2] + vp[3][0],
            vp[0][1] * point[0] + vp[1][1] * point[1] + vp[2][1] * point[2] + vp[3][1],
            vp[0][3] * point[0] + vp[1][3] * point[1] + vp[2][3] * point[2] + vp[3][3],
        ];
        [
            (clip[0] / clip[2] * 0.5 + 0.5) * size[0] as f32,
            (0.5 - clip[1] / clip[2] * 0.5) * size[1] as f32,
        ]
    }

    #[test]
    fn picking_ignores_scenery() {
        let scenery: Vec<Building> = one_app_city()
            .into_iter()
            .filter(|b| b.app.is_empty())
            .collect();
        let filler = &scenery[0];
        let size = [1000, 500];
        let at = project(filler.center, size, 2.0);
        assert_eq!(pick(&scenery, size, 2.0, at), None);
    }

    #[test]
    fn export_reads_back_a_square_frame() {
        // A machine with no usable adapter cannot exercise the GPU path.
        let instance = wgpu::Instance::default();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(
            &wgpu::RequestAdapterOptions::default(),
        )) else {
            return;
        };
        let Ok((device, queue)) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        else {
            return;
        };

        let mut renderer = CityRenderer::new(&device, wgpu::TextureFormat::Rgba8Unorm);
        let buildings = one_app_city();
        for blueprint in [false, true] {
            let look = CityLook {
                mood: Mood::Deep,
                intensity: 0.8,
                hour: 0.7,
                highlight: None,
                render_scale: 1.0,
                traffic: true,
                fps: 30,
                blueprint,
            };
            let (rgba, width, height) = renderer
                .export_rgba(&device, &queue, 256, 2.0, &look, &buildings)
                .expect("export reads back");
            assert_eq!((width, height), (256, 256));
            assert_eq!(rgba.len(), (256 * 256 * 4) as usize);
            assert!(rgba.chunks_exact(4).all(|p| p[3] == 255));
            assert!(
                rgba[4..].iter().any(|&b| b > 8),
                "the exported frame should not be empty"
            );
        }
    }

    #[test]
    fn picking_finds_the_tower_under_the_pointer() {
        let city = one_app_city();
        let size = [1000, 500];
        let at = project(city[0].center, size, 2.0);
        assert_eq!(pick(&city, size, 2.0, at), Some(0));
        // Empty sky above the skyline hits nothing.
        assert_eq!(pick(&city, size, 2.0, [8.0, 8.0]), None);
    }
}
