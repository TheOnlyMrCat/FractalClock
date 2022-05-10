use libc::c_void;
use pollster::FutureExt;
use raw_window_handle::HasRawWindowHandle;
use time::{OffsetDateTime, UtcOffset};
use wgpu::util::DeviceExt;

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,

    surface: wgpu::Surface,
    config: wgpu::SurfaceConfiguration,

    vertex_buffer: wgpu::Buffer,
    vertex_bind_group: wgpu::BindGroup,
    index_buffer: wgpu::Buffer,
    index_bind_group: wgpu::BindGroup,
    colour_buffer: wgpu::Buffer,
    colour_bind_group: wgpu::BindGroup,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    clock_buffer: wgpu::Buffer,

    initial_vertex_uniform_buffers: Vec<(wgpu::Buffer, wgpu::BindGroup)>,

    compute_uniform_bind_group_layout: wgpu::BindGroupLayout,
    compute_storage_bind_group_layout: wgpu::BindGroupLayout,
    vertex_storage_bind_group_layout: wgpu::BindGroupLayout,

    depth_texture: wgpu::Texture,
    depth_texture_view: wgpu::TextureView,
    resolve_texture: wgpu::Texture,
    resolve_texture_view: wgpu::TextureView,

    fractal_render_pipeline: wgpu::RenderPipeline,
    clock_render_pipeline: wgpu::RenderPipeline,
    vertices_compute_pipeline: wgpu::ComputePipeline,
    indices_compute_pipeline: wgpu::ComputePipeline,

    fractal_depth: u32,
}

impl Renderer {
    pub async fn new(
        fractal_depth: u32,
        window: &impl HasRawWindowHandle,
        (width, height): (u32, u32),
    ) -> Self {
        let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);

        let surface = unsafe { instance.create_surface(window) };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("FC Device"),
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .unwrap();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &config);

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("FC Vertex Buffer"),
            size: vertex_count_for_depth(fractal_depth) * vertex_size() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("FC Index Buffer"),
            size: vertex_count_for_depth(fractal_depth) * vertex_size() as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let colour_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("FC Colour Buffer"),
            size: std::mem::size_of::<[f64; 4]>() as u64 * fractal_depth as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("FC Camera Buffer"),
            size: std::mem::size_of::<[f64; 16]>() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });

        let clock_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("FC Clock Vertex Buffer"),
            size: std::mem::size_of::<ClockRay>() as u64 * 75,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let compute_storage_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("FC Storage Bind Group Layout (Compute)"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let vertex_storage_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("FC Storage Bind Group Layout (Vertex)"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let compute_uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("FC Uniform Bind Group Layout (Compute)"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let vertex_uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("FC Uniform Bind Group Layout (Vertex)"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let vertex_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("FC Vertex Bind Group"),
            layout: &compute_storage_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(vertex_buffer.as_entire_buffer_binding()),
            }],
        });

        let index_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("FC Index Bind Group"),
            layout: &compute_storage_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(index_buffer.as_entire_buffer_binding()),
            }],
        });

        let colour_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("FC Colour Bind Group"),
            layout: &vertex_storage_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(colour_buffer.as_entire_buffer_binding()),
            }],
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("FC Camera Bind Group"),
            layout: &vertex_uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(camera_buffer.as_entire_buffer_binding()),
            }],
        });

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("FC Depth Texture"),
            size: wgpu::Extent3d {
                width: 640,
                height: 480,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        });
        let depth_texture_view = depth_texture.create_view(&Default::default());

        let resolve_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("FC MSAA Resolve Texture"),
            size: wgpu::Extent3d {
                width: 640,
                height: 480,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        });
        let resolve_texture_view = resolve_texture.create_view(&Default::default());

        let fractal_shader_module =
            device.create_shader_module(&wgpu::include_wgsl!("fractal.wgsl"));
        let clock_shader_module = device.create_shader_module(&wgpu::include_wgsl!("clock.wgsl"));
        let vertices_shader_module =
            device.create_shader_module(&wgpu::include_wgsl!("vertices.wgsl"));
        let indices_shader_module =
            device.create_shader_module(&wgpu::include_wgsl!("indices.wgsl"));

        let fractal_render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("FC Fractal Render Pipeline Layout"),
                bind_group_layouts: &[
                    &vertex_uniform_bind_group_layout,
                    &vertex_storage_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let fractal_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("FC Fractal Render Pipeline"),
                layout: Some(&fractal_render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &fractal_shader_module,
                    entry_point: "vs_main",
                    buffers: &[fractal_vertex_attributes()],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::LineList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: 4,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fractal_shader_module,
                    entry_point: "fs_main",
                    targets: &[wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8Unorm,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    }],
                }),
                multiview: None,
            });

        let clock_render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("FC Fractal Render Pipeline Layout"),
                bind_group_layouts: &[
                    &vertex_uniform_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let clock_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("FC Clock Render Pipeline"),
                layout: Some(&clock_render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &clock_shader_module,
                    entry_point: "vs_main",
                    buffers: &[ClockRay::vertex_attributes()],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Always,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: 4,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &clock_shader_module,
                    entry_point: "fs_main",
                    targets: &[wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8Unorm,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    }],
                }),
                multiview: None,
            });

        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("FC Compute Pipeline Layout"),
                bind_group_layouts: &[
                    &compute_storage_bind_group_layout,
                    &compute_uniform_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let vertices_compute_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("FC Vertices Compute Pipeline"),
                layout: Some(&compute_pipeline_layout),
                module: &vertices_shader_module,
                entry_point: "main",
            });

        let indices_compute_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("FC Indices Compute Pipeline"),
                layout: Some(&compute_pipeline_layout),
                module: &indices_shader_module,
                entry_point: "main",
            });

        let initial_vertex_uniform_buffers = (0..fractal_depth)
            .map(|_| {
                let uniform_buffer =
                    device
                        .create_buffer(&wgpu::BufferDescriptor {
                            label: Some("FC Uniform Buffer"),
                            size: std::mem::size_of::<[f32; 4]>() as u64,
                            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        });

                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("FC Uniform Buffer Bind Group"),
                    layout: &compute_uniform_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(
                            uniform_buffer.as_entire_buffer_binding(),
                        ),
                    }],
                });

                (uniform_buffer, bind_group)
            })
            .collect::<Vec<_>>();

        let mut renderer = Self {
            device,
            queue,
            surface,
            config,
            vertex_buffer,
            vertex_bind_group,
            index_buffer,
            index_bind_group,
            colour_buffer,
            colour_bind_group,
            camera_buffer,
            camera_bind_group,
            clock_buffer,
            initial_vertex_uniform_buffers,
            compute_uniform_bind_group_layout,
            compute_storage_bind_group_layout,
            vertex_storage_bind_group_layout,
            depth_texture,
            depth_texture_view,
            resolve_texture,
            resolve_texture_view,
            fractal_render_pipeline,
            clock_render_pipeline,
            vertices_compute_pipeline,
            indices_compute_pipeline,
            fractal_depth,
        };

        renderer.prepare_indices();
        renderer.resize((width, height));
        renderer.write_ticks();
        renderer
    }

    fn prepare_indices(&self) {
        let workgroup_size = workgroup_size_for_depth(self.fractal_depth) as u32;
        let workgroup_y = workgroup_size / 256 + 1;
        let workgroup_x = workgroup_size % 256 + 1;

        //TODO: More offsets required? Consider push constants if on native?
        let null_offset = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Null offset"),
                contents: &0u32.to_ne_bytes(),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let null_offset_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Null offset bind group"),
            layout: &self.compute_uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(null_offset.as_entire_buffer_binding()),
            }],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("FC Indices Compute Encoder"),
            });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("FC Indices Compute Pass"),
        });
        pass.set_pipeline(&self.indices_compute_pipeline);
        pass.set_bind_group(0, &self.index_bind_group, &[]);
        pass.set_bind_group(1, &null_offset_bind_group, &[]);
        pass.dispatch(workgroup_x, workgroup_y, 1);
        drop(pass);

        self.queue.submit([encoder.finish()]);
    }

    pub fn set_depth(&mut self, fractal_depth: u32) {
        let fractal_depth = fractal_depth.max(1);
        self.fractal_depth = fractal_depth;

        self.vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("FC Vertex Buffer"),
            size: vertex_count_for_depth(fractal_depth) * vertex_size() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        self.vertex_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("FC Vertex Bind Group"),
            layout: &self.compute_storage_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(
                    self.vertex_buffer.as_entire_buffer_binding(),
                ),
            }],
        });

        self.index_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("FC Index Buffer"),
            size: vertex_count_for_depth(fractal_depth) * vertex_size() as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        self.index_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("FC Index Bind Group"),
            layout: &self.compute_storage_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(
                    self.index_buffer.as_entire_buffer_binding(),
                ),
            }],
        });

        self.colour_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("FC Colour Buffer"),
            size: std::mem::size_of::<[f64; 4]>() as u64 * fractal_depth as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        self.colour_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("FC Colour Bind Group"),
            layout: &self.vertex_storage_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(
                    self.colour_buffer.as_entire_buffer_binding(),
                ),
            }],
        });

        self.initial_vertex_uniform_buffers = (0..fractal_depth)
            .map(|depth| {
                let uniform_buffer =
                    self.device
                        .create_buffer(&wgpu::BufferDescriptor {
                            label: Some(&format!("FC Uniform Buffer {}", depth)),
                            size: std::mem::size_of::<[f32; 4]>() as u64,
                            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        });

                let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("FC Uniform Buffer {} Bind Group", depth)),
                    layout: &self.compute_uniform_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(
                            uniform_buffer.as_entire_buffer_binding(),
                        ),
                    }],
                });

                (uniform_buffer, bind_group)
            })
            .collect::<Vec<_>>();

        self.prepare_indices();
    }

    fn write_ticks(&self) {
        self.queue.write_buffer(
            &self.clock_buffer,
            std::mem::size_of::<ClockRay>() as u64 * 3,
            bytemuck::cast_slice(
                &(0..12)
                    .flat_map(|i| {
                        std::iter::once(ClockRay {
                            start: 0.45,
                            end: 0.5,
                            direction: (i as f32 * std::f32::consts::TAU / 12.0),
                            half_thickness: 0.004784689,
                        })
                        .chain((0..5).map(move |j| ClockRay {
                            start: 0.475,
                            end: 0.5,
                            direction: (i as f32 * std::f32::consts::TAU / 12.0)
                                + (j as f32 * std::f32::consts::TAU / 60.0),
                            half_thickness: 0.0023923445,
                        }))
                    })
                    .collect::<Vec<_>>(),
            ),
        );
    }

    pub fn resize(&mut self, (width, height): (u32, u32)) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);

        self.depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("FC Depth Texture"),
            format: wgpu::TextureFormat::Depth32Float,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        });
        self.depth_texture_view = self.depth_texture.create_view(&Default::default());
        self.resolve_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("FC MSAA Resolve Texture"),
            format: wgpu::TextureFormat::Bgra8Unorm,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        });
        self.resolve_texture_view = self.resolve_texture.create_view(&Default::default());

        let matrix = if width > height {
            let aspect = width as f32 / height as f32;
            [
                [1.0 / (1.35 * aspect), 0.0, 0.0, 0.0],
                [0.0, 1.0 / 1.35, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]
        } else {
            let aspect = height as f32 / width as f32;
            [
                [1.0 / 1.35, 0.0, 0.0, 0.0],
                [0.0, 1.0 / (1.35 * aspect), 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]
        };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&matrix));
    }

    pub fn render(&self) {
        let (hr, min, sec, ms) = OffsetDateTime::now_utc()
            .to_offset(local_timezone())
            .to_hms_milli();

        let second_frac = (sec as f32 / 60.0) + (ms as f32 / 60_000.0);
        let minute_frac = (min as f32 / 60.0) + (second_frac / 60.0);
        let hour_frac = (hr as f32 / 12.0) + ((minute_frac / 60.0) * (60.0 / 12.0));

        // Function adapted from https://github.com/HackerPoet/FractalClock
        let colour_time = ms as f32 / 1000.0 + sec as f32 + min as f32 * 60.0 + hr as f32 * 3600.0;
        let r1 = (colour_time * 0.017).sin() * 0.5 + 0.5;
        let r2 = (colour_time * -0.011).sin() * 0.5 + 0.5;
        let r3 = (colour_time * 0.003).sin() * 0.5 + 0.5;

        let mut colours = Vec::with_capacity(self.fractal_depth as usize);
        for i in 1..=self.fractal_depth {
            let a = (self.fractal_depth - i) as f32 / self.fractal_depth as f32;
            let h = r2 + 0.5 * a;
            let s = 0.5 + 0.5 * r3 - 0.5 * (1.0 - a);
            let v = 0.2 + 0.3 * r1 + 0.4 * a;
            if i == self.fractal_depth {
                let [r, g, b] = rgb_from_hsl((h, 1.0, 0.5));
                colours.push([r, g, b, 0.5]);
            } else {
                let [r, g, b] = rgb_from_hsl((h, s, v));
                colours.push([r, g, b, 1.0]);
            }
        }
        self.queue
            .write_buffer(&self.colour_buffer, 0, bytemuck::cast_slice(&colours));

        self.queue.write_buffer(
            &self.clock_buffer,
            0,
            bytemuck::cast_slice(&[
                ClockRay {
                    start: 0.0,
                    end: 0.175,
                    direction: hour_frac * std::f32::consts::TAU,
                    half_thickness: 0.02085,
                },
                ClockRay {
                    start: 0.0,
                    end: 0.4,
                    direction: minute_frac * std::f32::consts::TAU,
                    half_thickness: 0.0095,
                },
                ClockRay {
                    start: 0.0,
                    end: 0.4,
                    direction: second_frac * std::f32::consts::TAU,
                    half_thickness: 0.005,
                },
            ]),
        );

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("FC Render Encoder"),
            });

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("FC Render Compute Pass"),
        });
        pass.set_pipeline(&self.vertices_compute_pipeline);
        pass.set_bind_group(0, &self.vertex_bind_group, &[]);
        for (i, (buffer, bind_group)) in self.initial_vertex_uniform_buffers.iter().enumerate() {
            self.queue.write_buffer(buffer, 0, bytemuck::cast_slice(&[
                i as f32,
                second_frac * std::f32::consts::TAU,
                minute_frac * std::f32::consts::TAU,
                hour_frac * std::f32::consts::TAU,
            ]));
            pass.set_bind_group(1, bind_group, &[]);

            let workgroup_size = workgroup_size_for_depth((i + 1) as _) as u32;
            let workgroup_y = workgroup_size / 256 + 1;
            let workgroup_x = workgroup_size % 256 + 1;

            pass.dispatch(workgroup_x, workgroup_y, 1);
        }
        drop(pass);

        let surface_texture = self.surface.get_current_texture().unwrap();
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("FC Render Render Pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: &self.resolve_texture_view,
                resolve_target: Some(&view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.1,
                        b: 0.1,
                        a: 1.0,
                    }),
                    store: false,
                },
            }],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_texture_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: false,
                }),
                stencil_ops: None,
            }),
        });

        if self.fractal_depth > 1 {
            pass.set_pipeline(&self.fractal_render_pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(1, &self.colour_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(
                6..(vertex_count_for_depth(self.fractal_depth) * 2 - 2) as _,
                0,
                0..1,
            );
        }

        pass.set_pipeline(&self.clock_render_pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_vertex_buffer(0, self.clock_buffer.slice(..));
        pass.draw(0..4, 0..75);

        drop(pass);

        self.queue.submit([encoder.finish()]);

        surface_texture.present();
    }
}

fn vertex_size() -> usize {
    std::mem::size_of::<[f32; 2]>() + std::mem::size_of::<f32>() * 2
}

pub fn fractal_vertex_attributes() -> wgpu::VertexBufferLayout<'static> {
    use std::mem::size_of;

    wgpu::VertexBufferLayout {
        array_stride: vertex_size() as _,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: size_of::<[f32; 2]>() as _,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: size_of::<[f32; 3]>() as _,
                shader_location: 2,
            },
        ],
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ClockRay {
    start: f32,
    end: f32,
    direction: f32,
    half_thickness: f32,
}

impl ClockRay {
    fn vertex_attributes() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ClockRay>() as _,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: std::mem::size_of::<f32>() as _,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: (std::mem::size_of::<f32>() * 2) as _,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: (std::mem::size_of::<f32>() * 3) as _,
                    shader_location: 3,
                },
            ],
        }
    }
}

pub fn workgroup_size_for_depth(depth: u32) -> u64 {
    3_u64.pow(depth - 1)
}

pub fn vertex_count_for_depth(depth: u32) -> u64 {
    3_u64.pow(depth + 1) / 2 // OEIS A003462
}

/// All ranges in 0-1, rgb is linear.
pub fn rgb_from_hsv((h, s, v): (f32, f32, f32)) -> [f32; 3] {
    let h = (h.fract() + 1.0).fract(); // wrap
    let s = s.clamp(0.0, 1.0);

    let f = h * 6.0 - (h * 6.0).floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);

    match (h * 6.0).floor() as i32 % 6 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        5 => [v, p, q],
        _ => unreachable!(),
    }
}

pub fn rgb_from_hsl((h, s, l): (f32, f32, f32)) -> [f32; 3] {
    let a = s * l.min(1.0 - l);
    let f = |n: f32| {
        let k = (n + h / (1.0 / 12.0)) % 12.0;
        l - a * (k - 3.0).min(9.0 - k).min(1.0).max(-1.0)
    };
    [f(0.0), f(8.0), f(4.0)]
}

#[cfg(not(unix))]
fn local_timezone() -> UtcOffset {
    UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC)
}

#[cfg(unix)]
fn local_timezone() -> UtcOffset {
    // Workaround for `time` not providing local time on unix systems.
    let mut time = 0;
    let tm = unsafe {
        // SAFETY: We know our app isn't multithreaded, so we can just use these functions.
        libc::time(&mut time);
        *libc::localtime(&time)
    };
    UtcOffset::from_whole_seconds(tm.tm_gmtoff as i32).unwrap_or(UtcOffset::UTC)
}

/// ## Safety
/// `ns_view` must be a valid pointer to an active `NSView`, and must remain valid for the
/// lifetime of the returned `Renderer`.
#[no_mangle]
pub unsafe extern "C" fn fc_create_nsview(
    ns_view: *mut c_void,
    depth: u32,
    width: u32,
    height: u32,
) -> *mut Renderer {
    let mut handle = raw_window_handle::AppKitHandle::empty();
    handle.ns_view = ns_view;

    struct Handle {
        handle: raw_window_handle::RawWindowHandle,
    }

    unsafe impl HasRawWindowHandle for Handle {
        fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle {
            self.handle
        }
    }

    let handle = Handle {
        handle: raw_window_handle::RawWindowHandle::AppKit(handle),
    };

    let renderer = Box::new(Renderer::new(depth, &handle, (width, height)).block_on());
    Box::into_raw(renderer)
}

/// ## Safety
/// The pointer must have been returned by a call to `create_renderer`.
#[no_mangle]
pub unsafe extern "C" fn fc_destroy(renderer: *mut Renderer) {
    Box::from_raw(renderer);
}

/// ## Safety
/// The pointer must have been returned by a call to `create_renderer`.
/// 
/// `libc::setenv` must not cause a data race with `libc::localtime`.
#[no_mangle]
pub unsafe extern "C" fn fc_render(renderer: *mut Renderer) {
    (*renderer).render();
}

/// ## Safety
/// The pointer must have been returned by a call to `create_renderer`
#[no_mangle]
pub unsafe extern "C" fn fc_set_depth(renderer: *mut Renderer, depth: u32) {
    (*renderer).set_depth(depth);
}

/// ## Safety
/// The pointer must have been returned by a call to `create_renderer`.
#[no_mangle]
pub unsafe extern "C" fn fc_resize(renderer: *mut Renderer, width: u32, height: u32) {
    (*renderer).resize((width, height));
}
