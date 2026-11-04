use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, mpsc};

use tracing::{info, warn};
use wgpu::util::{DeviceExt, TextureDataOrder};

use foundry::systems::raster::Vertex;

use super::gpu::{Scene, SceneSheet, SceneSlot};

const SHADER: &str = r"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@group(0) @binding(0) var sheet: texture_2d<f32>;
@group(0) @binding(1) var sheet_sampler: sampler;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip = vec4<f32>(input.position, 0.0, 1.0);
    out.uv = input.uv;
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(sheet, sheet_sampler, input.uv) * input.color;
}
";
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const WHITE: [u8; 4] = [0xff; 4];
const BYTES_PER_PIXEL: u32 = 4;
const BLENDS: [(wgpu::BlendFactor, wgpu::BlendFactor); 4] = [
    (wgpu::BlendFactor::One, wgpu::BlendFactor::OneMinusSrcAlpha),
    (wgpu::BlendFactor::One, wgpu::BlendFactor::One),
    (wgpu::BlendFactor::Dst, wgpu::BlendFactor::Zero),
    (wgpu::BlendFactor::One, wgpu::BlendFactor::OneMinusSrc),
];

struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    readback: wgpu::Buffer,
    width: u32,
    height: u32,
    stride: u32,
}

struct Offscreen {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipelines: [wgpu::RenderPipeline; 4],
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    white: wgpu::BindGroup,
    sheets: HashMap<usize, (Arc<[u8]>, wgpu::BindGroup)>,
    target: Option<Target>,
    vertices: Option<wgpu::Buffer>,
    capacity: u64,
}

enum Slot {
    Untried,
    Ready(Box<Offscreen>),
    Failed,
}

thread_local! {
    static SLOT: RefCell<Slot> = const { RefCell::new(Slot::Untried) };
}

fn with_offscreen<T>(work: impl FnOnce(&mut Offscreen) -> T) -> Option<T> {
    SLOT.with(|slot| {
        let mut held = slot.borrow_mut();

        if matches!(*held, Slot::Untried) {
            *held = match Offscreen::new() {
                Ok(offscreen) => Slot::Ready(Box::new(offscreen)),
                Err(err) => {
                    warn!("emu: frames fall back to the CPU, no GPU device: {err}");
                    Slot::Failed
                }
            };
        }

        match &mut *held {
            Slot::Ready(offscreen) => Some(work(offscreen)),
            Slot::Untried | Slot::Failed => None,
        }
    })
}

pub(super) fn available() -> bool {
    with_offscreen(|_| ()).is_some()
}

pub(super) fn render(scene: &Scene) -> Option<Vec<u8>> {
    if !crate::ios::app_active() {
        return None;
    }

    with_offscreen(|offscreen| offscreen.draw(scene))?.inspect_err(|err| warn!("emu: a GPU frame could not be drawn: {err}")).ok()
}

pub(super) fn forget() {
    with_offscreen(|offscreen| {
        offscreen.sheets.clear();
        offscreen.target = None;
    });
}

impl Offscreen {
    fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).map_err(|err| err.to_string())?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { required_limits: adapter.limits(), ..wgpu::DeviceDescriptor::default() })).map_err(|err| err.to_string())?;

        device.set_device_lost_callback(|reason, message| warn!(?reason, message, "emu: the GPU device was lost"));

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("scene"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene_sheet"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("scene"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let attributes = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];
        let pipelines = BLENDS.map(|(source, destination)| {
            let carried = source != wgpu::BlendFactor::One || destination != wgpu::BlendFactor::OneMinusSrcAlpha;
            let (source_alpha, destination_alpha) = if carried { (wgpu::BlendFactor::Zero, wgpu::BlendFactor::One) } else { (wgpu::BlendFactor::One, wgpu::BlendFactor::OneMinusSrcAlpha) };

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("scene"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout { array_stride: size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attributes })],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: FORMAT,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent { src_factor: source, dst_factor: destination, operation: wgpu::BlendOperation::Add },
                            alpha: wgpu::BlendComponent { src_factor: source_alpha, dst_factor: destination_alpha, operation: wgpu::BlendOperation::Add },
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..wgpu::SamplerDescriptor::default() });
        let white = bind_sheet(&device, &queue, &layout, &sampler, 1, 1, &WHITE);

        info!(backend = ?adapter.get_info().backend, "emu: frames draw on the GPU and read back");

        Ok(Self { device, queue, pipelines, layout, sampler, white, sheets: HashMap::new(), target: None, vertices: None, capacity: 0 })
    }

    fn sized(&mut self, width: u32, height: u32) {
        if self.target.as_ref().is_some_and(|held| held.width == width && held.height == height) {
            return;
        }

        let stride = (width * BYTES_PER_PIXEL).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene_target"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene_readback"),
            size: u64::from(stride) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        self.target = Some(Target { texture, view, readback, width, height, stride });
    }

    fn sheet(&mut self, sheet: &SceneSheet) -> wgpu::BindGroup {
        let key = Arc::as_ptr(&sheet.pixels).cast::<u8>() as usize;

        if let Some((_, bound)) = self.sheets.get(&key) {
            return bound.clone();
        }

        let bound = bind_sheet(&self.device, &self.queue, &self.layout, &self.sampler, sheet.width, sheet.height, &sheet.pixels);

        self.sheets.insert(key, (Arc::clone(&sheet.pixels), bound.clone()));

        bound
    }

    fn draw(&mut self, scene: &Scene) -> Result<Vec<u8>, String> {
        self.sized(scene.width, scene.height);

        if scene.slot == SceneSlot::Viewer {
            let kept: Vec<usize> = scene.sheets.iter().map(|sheet| Arc::as_ptr(&sheet.pixels).cast::<u8>() as usize).collect();

            self.sheets.retain(|key, _| kept.contains(key));
        }

        let white = self.white.clone();
        let bindings: Vec<wgpu::BindGroup> = scene
            .runs
            .iter()
            .map(|run| run.sheet.and_then(|index| scene.sheets.get(index)).map_or_else(|| white.clone(), |sheet| self.sheet(sheet)))
            .collect();
        let bytes: &[u8] = bytemuck::cast_slice(&scene.vertices);

        if self.capacity < bytes.len() as u64 || self.vertices.is_none() {
            self.capacity = (bytes.len() as u64).max(1).next_power_of_two();
            self.vertices = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("scene_vertices"),
                size: self.capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        let (Some(target), Some(vertices)) = (self.target.as_ref(), self.vertices.as_ref()) else {
            return Err("the render target is missing".to_owned());
        };

        self.queue.write_buffer(vertices, 0, bytes);

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("scene") });

        {
            let [red, green, blue, alpha] = scene.clear.map(f64::from);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: red, g: green, b: blue, a: alpha }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_vertex_buffer(0, vertices.slice(..));

            for (run, binding) in scene.runs.iter().zip(&bindings) {
                pass.set_pipeline(&self.pipelines[if run.blend <= 3 { usize::from(run.blend) } else { 0 }]);
                pass.set_bind_group(0, binding, &[]);
                pass.draw(run.range.start as u32..run.range.end as u32, 0..1);
            }
        }

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &target.texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo {
                buffer: &target.readback,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(target.stride), rows_per_image: Some(target.height) },
            },
            wgpu::Extent3d { width: target.width, height: target.height, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);

        let slice = target.readback.slice(..);
        let (sender, receiver) = mpsc::channel();

        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|err| err.to_string())?;
        receiver.recv().map_err(|err| err.to_string())?.map_err(|err| err.to_string())?;

        let row = (target.width * BYTES_PER_PIXEL) as usize;
        let mut pixels = Vec::with_capacity(row * target.height as usize);

        let mapped = slice.get_mapped_range().map_err(|err| err.to_string())?;

        for line in mapped.chunks_exact(target.stride as usize) {
            pixels.extend_from_slice(&line[..row]);
        }

        drop(mapped);
        target.readback.unmap();

        Ok(pixels)
    }
}

fn bind_sheet(device: &wgpu::Device, queue: &wgpu::Queue, layout: &wgpu::BindGroupLayout, sampler: &wgpu::Sampler, width: u32, height: u32, pixels: &[u8]) -> wgpu::BindGroup {
    let texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("scene_sheet"),
            size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        TextureDataOrder::LayerMajor,
        pixels,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("scene_sheet"),
        layout,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) }],
    })
}
