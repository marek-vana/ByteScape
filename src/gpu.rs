//! Custom wgpu material pass: Retina-resolved microgrid, optical grain and a quiet sweep.
use eframe::{
    egui,
    egui_wgpu::{self, wgpu},
};
use wgpu::util::DeviceExt as _;
struct Resources {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}
struct Grid {
    time: f32,
}
pub fn initialize(cc: &eframe::CreationContext<'_>) {
    let Some(state) = &cc.wgpu_render_state else {
        return;
    };
    let shader = state
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ByteScape optical cartography material"),
            source: wgpu::ShaderSource::Wgsl(include_str!("atmosphere.wgsl").into()),
        });
    let layout = state
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ByteScape material clock"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(16),
                },
                count: None,
            }],
        });
    let uniform = state
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ByteScape material time"),
            contents: &[0_u8; 16],
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
    let bind_group = state.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ByteScape material"),
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform.as_entire_binding(),
        }],
    });
    let pipeline_layout = state
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ByteScape atmosphere layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
    let pipeline = state
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ByteScape atmosphere pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                targets: &[Some(state.target_format.into())],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 4,
                ..Default::default()
            },
            multiview_mask: None,
            cache: None,
        });
    state.renderer.write().callback_resources.insert(Resources {
        pipeline,
        uniform,
        bind_group,
    });
}
pub fn background(painter: &egui::Painter, rect: egui::Rect, time: f32) {
    painter.add(egui_wgpu::Callback::new_paint_callback(rect, Grid { time }));
}
impl egui_wgpu::CallbackTrait for Grid {
    fn prepare(
        &self,
        _: &wgpu::Device,
        queue: &wgpu::Queue,
        _: &egui_wgpu::ScreenDescriptor,
        _: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(r) = resources.get::<Resources>() {
            let mut bytes = [0_u8; 16];
            bytes[..4].copy_from_slice(&self.time.to_ne_bytes());
            queue.write_buffer(&r.uniform, 0, &bytes);
        }
        Vec::new()
    }
    fn paint(
        &self,
        _: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(r) = resources.get::<Resources>() {
            pass.set_pipeline(&r.pipeline);
            pass.set_bind_group(0, &r.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}
