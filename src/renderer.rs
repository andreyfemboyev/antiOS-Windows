use crate::{
    configuration::{Backend, Quality},
    typography::Typography,
};
use anyhow::{anyhow, bail, ensure, Context, Result};
use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use std::{
    path::Path,
    sync::{mpsc, Arc},
};
use wgpu as gpu;
use winit::window::Window;

pub const CANVAS: Vec2 = Vec2::new(1440., 900.);
pub const STAGE_ORIGIN: Vec2 = Vec2::new(58., 128.);
pub const STAGE_SIZE: Vec2 = Vec2::new(1324., 647.);
const FORMAT: gpu::TextureFormat = gpu::TextureFormat::Rgba8Unorm;
const ATTRIBUTES: [gpu::VertexAttribute; 4] =
    gpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4];

#[derive(Clone, Copy)]
pub struct Color(pub [f32; 4]);

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r as f32 / 255., g as f32 / 255., b as f32 / 255., 1.])
    }
    pub fn alpha(mut self, alpha: f32) -> Self {
        self.0[3] *= alpha.clamp(0., 1.);
        self
    }
}

pub const VOID: Color = Color::rgb(8, 8, 8);
pub const SURFACE: Color = Color::rgb(20, 20, 20);
pub const PAPER: Color = Color::rgb(232, 230, 225);
pub const MUTED: Color = Color::rgb(119, 119, 115);
pub const SIGNAL: Color = Color::rgb(184, 255, 54);
const CLEAR: gpu::Color = gpu::Color {
    r: 8. / 255.,
    g: 8. / 255.,
    b: 8. / 255.,
    a: 1.,
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Instance {
    shape: [f32; 4],
    style: [f32; 4],
    pigment: [f32; 4],
    bounds: [f32; 4],
}

#[derive(Default)]
pub struct Layer {
    pub instances: Vec<Instance>,
}

impl Layer {
    fn push(&mut self, center: Vec2, half: Vec2, style: [f32; 4], color: Color, bounds: [f32; 4]) {
        if color.0[3] > 0.002 && half.min_element() > 0. {
            self.instances.push(Instance {
                shape: [center.x, center.y, half.x, half.y],
                style,
                pigment: color.0,
                bounds,
            });
        }
    }
    pub fn rect(&mut self, position: Vec2, size: Vec2, color: Color) {
        self.push(position + size * 0.5, size * 0.5, [0.; 4], color, [0.; 4]);
    }
    pub fn outline(&mut self, position: Vec2, size: Vec2, width: f32, color: Color) {
        self.push(
            position + size * 0.5,
            size * 0.5,
            [0., 1., width, 0.],
            color,
            [0.; 4],
        );
    }
    pub fn dot(&mut self, center: Vec2, radius: f32, color: Color) {
        self.push(
            center,
            Vec2::splat(radius),
            [0., 2., 0., 0.],
            color,
            [0.; 4],
        );
    }
    pub fn ring(&mut self, center: Vec2, radius: Vec2, angle: f32, width: f32, color: Color) {
        self.push(center, radius, [angle, 4., width, 0.], color, [0.; 4]);
    }
    pub fn line(&mut self, a: Vec2, b: Vec2, width: f32, color: Color) {
        let direction = b - a;
        self.push(
            (a + b) * 0.5,
            Vec2::new((direction.length() + width) * 0.5, width * 0.5),
            [direction.y.atan2(direction.x), 3., 0., 0.],
            color,
            [0.; 4],
        );
    }
    pub fn glyph(&mut self, position: Vec2, size: Vec2, angle: f32, color: Color, uv: [f32; 4]) {
        self.push(
            position + size * 0.5,
            size * 0.5,
            [angle, 5., 0., 0.],
            color,
            uv,
        );
    }
    pub fn clear(&mut self) {
        self.instances.clear();
    }
}

pub fn scale(size: (u32, u32)) -> f32 {
    (size.0 as f32 / CANVAS.x).min(size.1 as f32 / CANVAS.y)
}

pub fn pointer(x: f64, y: f64, size: (u32, u32)) -> Option<Vec2> {
    let pixel = Vec2::new(x as f32, y as f32);
    let extent = Vec2::new(size.0 as f32, size.1 as f32);
    let point = (pixel - 0.5 * extent) / scale(size).max(0.001) / CANVAS + Vec2::splat(0.5);
    (point.min_element() >= 0. && point.max_element() <= 1.).then_some(point)
}

struct Picture {
    texture: gpu::Texture,
    view: gpu::TextureView,
}

impl Picture {
    fn new(device: &gpu::Device, size: (u32, u32), name: &str) -> Self {
        let texture = device.create_texture(&gpu::TextureDescriptor {
            label: Some(name),
            size: extent(size),
            mip_level_count: 1,
            sample_count: 1,
            dimension: gpu::TextureDimension::D2,
            format: FORMAT,
            usage: gpu::TextureUsages::RENDER_ATTACHMENT
                | gpu::TextureUsages::TEXTURE_BINDING
                | gpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&gpu::TextureViewDescriptor::default());
        Self { texture, view }
    }
}

fn extent(size: (u32, u32)) -> gpu::Extent3d {
    gpu::Extent3d {
        width: size.0,
        height: size.1,
        depth_or_array_layers: 1,
    }
}

struct Targets {
    scene: Picture,
    history: [Picture; 2],
    output: Picture,
    size: (u32, u32),
    scene_size: (u32, u32),
    front: usize,
    valid: bool,
}

impl Targets {
    fn new(device: &gpu::Device, size: (u32, u32), quality: Quality) -> Self {
        let scene_size = (
            (size.0 as f32 * quality.scale()).ceil() as u32,
            (size.1 as f32 * quality.scale()).ceil() as u32,
        );
        Self {
            scene: Picture::new(device, scene_size, "organisms"),
            history: [
                Picture::new(device, scene_size, "memory A"),
                Picture::new(device, scene_size, "memory B"),
            ],
            output: Picture::new(device, size, "artwork / readback"),
            size,
            scene_size,
            front: 0,
            valid: false,
        }
    }
}

struct Presentation {
    surface: gpu::Surface<'static>,
    config: gpu::SurfaceConfiguration,
    pipeline: gpu::RenderPipeline,
}

pub struct Renderer {
    device: gpu::Device,
    queue: gpu::Queue,
    targets: Targets,
    presentation: Option<Presentation>,
    geometry: gpu::RenderPipeline,
    history: gpu::RenderPipeline,
    composite: gpu::RenderPipeline,
    sampler: gpu::Sampler,
    geometry_groups: [gpu::BindGroup; 2],
    canvas_buffers: [gpu::Buffer; 2],
    signal: gpu::Buffer,
    instances: gpu::Buffer,
    instance_capacity: usize,
    pub quality: Quality,
    pub adapter: gpu::AdapterInfo,
    pub limit: u32,
}

impl Renderer {
    pub async fn new(
        window: Option<Arc<Window>>,
        size: (u32, u32),
        backend: Backend,
        requested: Quality,
        typography: &Typography,
    ) -> Result<Self> {
        let backends: Vec<gpu::Backends> = match backend {
            Backend::Auto => {
                if cfg!(windows) {
                    vec![
                        gpu::Backends::DX12,
                        gpu::Backends::VULKAN,
                        gpu::Backends::GL,
                    ]
                } else {
                    vec![gpu::Backends::VULKAN, gpu::Backends::GL]
                }
            }
            Backend::Vulkan => vec![gpu::Backends::VULKAN],
            Backend::Dx12 => vec![gpu::Backends::DX12],
            Backend::Gl => vec![gpu::Backends::GL],
        };
        let mut diagnostics = Vec::new();
        for api in backends {
            match Self::initialize(window.clone(), size, api, requested, typography).await {
                Ok(renderer) => return Ok(renderer),
                Err(error) => diagnostics.push(format!("{api:?}: {error:#}")),
            }
        }
        bail!("unable to initialize graphics. A Vulkan, D3D12 or OpenGL 3.3 / GLES 3.0 adapter is required. {}", diagnostics.join(" / "));
    }

    async fn initialize(
        window: Option<Arc<Window>>,
        size: (u32, u32),
        backends: gpu::Backends,
        requested: Quality,
        typography: &Typography,
    ) -> Result<Self> {
        let instance = gpu::Instance::new(&gpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });
        let surface = window
            .map(|window| instance.create_surface(window))
            .transpose()?;
        let options = gpu::RequestAdapterOptions {
            power_preference: gpu::PowerPreference::None,
            compatible_surface: surface.as_ref(),
            force_fallback_adapter: false,
        };
        let adapter = if let Some(adapter) = instance.request_adapter(&options).await {
            adapter
        } else {
            instance
                .request_adapter(&gpu::RequestAdapterOptions {
                    force_fallback_adapter: true,
                    ..options
                })
                .await
                .context("no compatible adapter")?
        };
        let info = adapter.get_info();
        let limits = gpu::Limits::downlevel_defaults().using_resolution(adapter.limits());
        let limit = limits.max_texture_dimension_2d;
        ensure!(
            size.0 <= limit && size.1 <= limit,
            "requested image exceeds the adapter's {}px texture limit",
            limit
        );
        let (device, queue) = adapter
            .request_device(
                &gpu::DeviceDescriptor {
                    label: Some("antiOS"),
                    required_features: gpu::Features::empty(),
                    required_limits: limits,
                    memory_hints: gpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await
            .context("requesting a graphics device")?;
        let quality = match requested {
            Quality::Auto
                if info.backend == gpu::Backend::Gl || info.device_type == gpu::DeviceType::Cpu =>
            {
                Quality::Low
            }
            Quality::Auto if info.device_type == gpu::DeviceType::IntegratedGpu => Quality::Medium,
            Quality::Auto => Quality::High,
            other => other,
        };

        let geometry_shader =
            device.create_shader_module(gpu::include_wgsl!("shaders/geometry.wgsl"));
        let post_shader = device.create_shader_module(gpu::include_wgsl!("shaders/feedback.wgsl"));
        let geometry = pipeline(&device, &geometry_shader, "fragment", FORMAT, true);
        let history = pipeline(&device, &post_shader, "history", FORMAT, false);
        let composite = pipeline(&device, &post_shader, "display", FORMAT, false);
        let targets = Targets::new(&device, size, quality);

        let atlas = device.create_texture(&gpu::TextureDescriptor {
            label: Some("embedded type atlas"),
            size: extent(typography.atlas_size),
            mip_level_count: 1,
            sample_count: 1,
            dimension: gpu::TextureDimension::D2,
            format: gpu::TextureFormat::R8Unorm,
            usage: gpu::TextureUsages::TEXTURE_BINDING | gpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            atlas.as_image_copy(),
            &typography.pixels,
            gpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(typography.atlas_size.0),
                rows_per_image: None,
            },
            extent(typography.atlas_size),
        );
        let glyph_view = atlas.create_view(&gpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&gpu::SamplerDescriptor {
            mag_filter: gpu::FilterMode::Linear,
            min_filter: gpu::FilterMode::Linear,
            ..Default::default()
        });
        let canvas_buffers = std::array::from_fn(|_| uniform(&device));
        let geometry_groups = canvas_buffers.each_ref().map(|canvas| {
            device.create_bind_group(&gpu::BindGroupDescriptor {
                label: Some("canvas"),
                layout: &geometry.get_bind_group_layout(0),
                entries: &[
                    gpu::BindGroupEntry {
                        binding: 0,
                        resource: canvas.as_entire_binding(),
                    },
                    bind_texture(1, &glyph_view),
                    bind_sampler(2, &sampler),
                ],
            })
        });
        let signal = uniform(&device);
        let instance_capacity = 2048;
        let instances = instance_buffer(&device, instance_capacity);
        let presentation = surface
            .map(|surface| {
                let mut config = surface
                    .get_default_config(&adapter, size.0, size.1)
                    .context("surface has no compatible format")?;
                let caps = surface.get_capabilities(&adapter);
                if let Some(&format) = caps.formats.iter().find(|format| !format.is_srgb()) {
                    config.format = format;
                }
                config.present_mode = gpu::PresentMode::Fifo;
                surface.configure(&device, &config);
                let pipeline = pipeline(&device, &post_shader, "display", config.format, false);
                Ok::<_, anyhow::Error>(Presentation {
                    surface,
                    config,
                    pipeline,
                })
            })
            .transpose()?;

        Ok(Self {
            device,
            queue,
            targets,
            presentation,
            geometry,
            history,
            composite,
            sampler,
            geometry_groups,
            canvas_buffers,
            signal,
            instances,
            instance_capacity,
            quality,
            adapter: info,
            limit,
        })
    }

    pub fn resize(&mut self, size: (u32, u32), quality: Quality) {
        if size.0 == 0 || size.1 == 0 {
            return;
        }
        let size = (size.0.min(self.limit), size.1.min(self.limit));
        if self.targets.size != size || self.quality != quality {
            self.targets = Targets::new(&self.device, size, quality);
        }
        self.quality = quality;
        if let Some(p) = &mut self.presentation {
            p.config.width = size.0;
            p.config.height = size.1;
            p.surface.configure(&self.device, &p.config);
        }
    }

    pub fn clear_history(&mut self) {
        self.targets.valid = false;
    }

    pub fn draw(&mut self, art: &Layer, chrome: &Layer, params: Frame, show: bool) -> Result<()> {
        let acquired = if show {
            if let Some(p) = &mut self.presentation {
                match p.surface.get_current_texture() {
                    Ok(texture) => Some(texture),
                    Err(gpu::SurfaceError::Lost | gpu::SurfaceError::Outdated) => {
                        p.surface.configure(&self.device, &p.config);
                        return Ok(());
                    }
                    Err(gpu::SurfaceError::Timeout | gpu::SurfaceError::Other) => return Ok(()),
                    Err(error) => return Err(error.into()),
                }
            } else {
                None
            }
        } else {
            None
        };
        let count = art.instances.len() + chrome.instances.len();
        if count > self.instance_capacity {
            self.instance_capacity = count.next_power_of_two();
            self.instances = instance_buffer(&self.device, self.instance_capacity);
        }
        let art_bytes = bytemuck::cast_slice(&art.instances);
        if !art_bytes.is_empty() {
            self.queue.write_buffer(&self.instances, 0, art_bytes);
        }
        if !chrome.instances.is_empty() {
            self.queue.write_buffer(
                &self.instances,
                art_bytes.len() as u64,
                bytemuck::cast_slice(&chrome.instances),
            );
        }
        for (buffer, size) in self
            .canvas_buffers
            .iter()
            .zip([self.targets.scene_size, self.targets.size])
        {
            self.queue.write_buffer(
                buffer,
                0,
                bytemuck::cast_slice(&[size.0 as f32, size.1 as f32, scale(size), 0.]),
            );
        }
        self.queue.write_buffer(
            &self.signal,
            0,
            bytemuck::cast_slice(&[
                params.time,
                params.fracture,
                if self.targets.valid {
                    params.persistence
                } else {
                    0.
                },
                0.0,
            ]),
        );
        let mut encoder = self
            .device
            .create_command_encoder(&gpu::CommandEncoderDescriptor::default());
        self.draw_shapes(
            &mut encoder,
            &self.targets.scene.view,
            0,
            0..art.instances.len() as u32,
            gpu::LoadOp::Clear(CLEAR),
        );
        let available = 1 - self.targets.front;
        self.fullscreen(
            &mut encoder,
            &self.history,
            &self.targets.history[available].view,
            &self.targets.scene.view,
            Some(&self.targets.history[self.targets.front].view),
            &self.signal,
        );
        self.fullscreen(
            &mut encoder,
            &self.composite,
            &self.targets.output.view,
            &self.targets.history[available].view,
            None,
            &self.signal,
        );
        self.draw_shapes(
            &mut encoder,
            &self.targets.output.view,
            1,
            art.instances.len() as u32..count as u32,
            gpu::LoadOp::Load,
        );

        if let (Some(frame), Some(p)) = (&acquired, &self.presentation) {
            let buffer = uniform(&self.device);
            self.queue.write_buffer(
                &buffer,
                0,
                bytemuck::cast_slice(&[
                    0.0_f32,
                    0.,
                    0.,
                    if p.config.format.is_srgb() { 1. } else { 0. },
                ]),
            );
            self.fullscreen(
                &mut encoder,
                &p.pipeline,
                &frame.texture.create_view(&Default::default()),
                &self.targets.output.view,
                None,
                &buffer,
            );
        }
        self.queue.submit([encoder.finish()]);
        if let Some(frame) = acquired {
            frame.present();
        }
        self.targets.front = available;
        self.targets.valid = true;
        Ok(())
    }

    fn draw_shapes(
        &self,
        encoder: &mut gpu::CommandEncoder,
        view: &gpu::TextureView,
        group: usize,
        range: std::ops::Range<u32>,
        load: gpu::LoadOp<gpu::Color>,
    ) {
        let mut pass = pass(encoder, view, load);
        if group == 0 {
            let size = self.targets.scene_size;
            let zoom = scale(size);
            let extent = Vec2::new(size.0 as f32, size.1 as f32);
            let margin = (extent - CANVAS * zoom) * 0.5;
            let corner = (margin + STAGE_ORIGIN * zoom).floor().max(Vec2::ZERO);
            let end = (margin + (STAGE_ORIGIN + STAGE_SIZE) * zoom)
                .ceil()
                .min(extent);
            pass.set_scissor_rect(
                corner.x as u32,
                corner.y as u32,
                (end.x - corner.x) as u32,
                (end.y - corner.y) as u32,
            );
        }
        if !range.is_empty() {
            pass.set_pipeline(&self.geometry);
            pass.set_bind_group(0, &self.geometry_groups[group], &[]);
            pass.set_vertex_buffer(0, self.instances.slice(..));
            pass.draw(0..6, range);
        }
    }

    fn fullscreen(
        &self,
        encoder: &mut gpu::CommandEncoder,
        pipeline: &gpu::RenderPipeline,
        destination: &gpu::TextureView,
        current: &gpu::TextureView,
        previous: Option<&gpu::TextureView>,
        signal: &gpu::Buffer,
    ) {
        let mut bindings = vec![bind_texture(0, current), bind_sampler(2, &self.sampler)];
        if let Some(view) = previous {
            bindings.push(bind_texture(1, view));
        }
        bindings.push(gpu::BindGroupEntry {
            binding: 3,
            resource: signal.as_entire_binding(),
        });
        let group = self.device.create_bind_group(&gpu::BindGroupDescriptor {
            label: Some("composition"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &bindings,
        });
        let mut pass = pass(encoder, destination, gpu::LoadOp::Clear(CLEAR));
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let size = self.targets.size;
        let row = 4 * size.0;
        let padded =
            row.div_ceil(gpu::COPY_BYTES_PER_ROW_ALIGNMENT) * gpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&gpu::BufferDescriptor {
            label: Some("print readback"),
            size: u64::from(padded) * u64::from(size.1),
            usage: gpu::BufferUsages::COPY_DST | gpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut copy = self
            .device
            .create_command_encoder(&gpu::CommandEncoderDescriptor::default());
        copy.copy_texture_to_buffer(
            self.targets.output.texture.as_image_copy(),
            gpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: gpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: None,
                },
            },
            extent(size),
        );
        self.queue.submit([copy.finish()]);
        let range = buffer.slice(..);
        let (send, receive) = mpsc::sync_channel(1);
        range.map_async(gpu::MapMode::Read, move |result| {
            let _ = send.send(result);
        });
        self.device.poll(gpu::Maintain::Wait);
        receive
            .recv()
            .context("graphics readback was interrupted")??;
        let bytes = range.get_mapped_range();
        crate::export::png(
            path,
            size,
            bytes
                .chunks_exact(padded as usize)
                .map(|line| &line[..row as usize]),
        )
    }

    pub fn print(
        &mut self,
        path: &Path,
        params: Frame,
        art: &Layer,
        chrome: &Layer,
        size: (u32, u32),
    ) -> Result<()> {
        if size.0.max(size.1) > self.limit {
            return Err(anyhow!("print dimensions exceed graphics limit"));
        }
        let temporary = Targets::new(&self.device, size, Quality::High);
        let live = std::mem::replace(&mut self.targets, temporary);
        let result = self
            .draw(art, chrome, params, false)
            .and_then(|()| self.save(path));
        self.targets = live;
        result
    }
}

#[derive(Clone, Copy)]
pub struct Frame {
    pub time: f32,
    pub fracture: f32,
    pub persistence: f32,
}

fn pass<'a>(
    encoder: &'a mut gpu::CommandEncoder,
    view: &gpu::TextureView,
    load: gpu::LoadOp<gpu::Color>,
) -> gpu::RenderPass<'a> {
    encoder.begin_render_pass(&gpu::RenderPassDescriptor {
        color_attachments: &[Some(gpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: gpu::Operations {
                load,
                store: gpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    })
}

fn bind_texture(binding: u32, texture: &gpu::TextureView) -> gpu::BindGroupEntry<'_> {
    gpu::BindGroupEntry {
        binding,
        resource: gpu::BindingResource::TextureView(texture),
    }
}
fn bind_sampler(binding: u32, sampler: &gpu::Sampler) -> gpu::BindGroupEntry<'_> {
    gpu::BindGroupEntry {
        binding,
        resource: gpu::BindingResource::Sampler(sampler),
    }
}

fn uniform(device: &gpu::Device) -> gpu::Buffer {
    device.create_buffer(&gpu::BufferDescriptor {
        label: Some("frame uniform"),
        size: 16,
        usage: gpu::BufferUsages::UNIFORM | gpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn instance_buffer(device: &gpu::Device, count: usize) -> gpu::Buffer {
    device.create_buffer(&gpu::BufferDescriptor {
        label: Some("instanced primitives and glyphs"),
        size: (count * size_of::<Instance>()) as u64,
        usage: gpu::BufferUsages::VERTEX | gpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn pipeline(
    device: &gpu::Device,
    shader: &gpu::ShaderModule,
    fragment: &str,
    format: gpu::TextureFormat,
    geometry: bool,
) -> gpu::RenderPipeline {
    let attributes = [gpu::VertexBufferLayout {
        array_stride: size_of::<Instance>() as u64,
        step_mode: gpu::VertexStepMode::Instance,
        attributes: &ATTRIBUTES,
    }];
    device.create_render_pipeline(&gpu::RenderPipelineDescriptor {
        label: Some(fragment),
        layout: None,
        vertex: gpu::VertexState {
            module: shader,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: if geometry { &attributes } else { &[] },
        },
        fragment: Some(gpu::FragmentState {
            module: shader,
            entry_point: Some(fragment),
            compilation_options: Default::default(),
            targets: &[Some(gpu::ColorTargetState {
                format,
                blend: geometry.then_some(gpu::BlendState::ALPHA_BLENDING),
                write_mask: gpu::ColorWrites::ALL,
            })],
        }),
        primitive: gpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: gpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}
