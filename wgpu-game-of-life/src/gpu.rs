use wasm_bindgen::JsCast;
use wgpu::util::DeviceExt;

pub struct Simulation {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    compute_pipeline: wgpu::ComputePipeline,
    render_pipeline: wgpu::RenderPipeline,
    cell_buffers: [wgpu::Buffer; 2],
    #[allow(dead_code)]
    uniform_buffer: wgpu::Buffer,
    compute_bind_groups: [wgpu::BindGroup; 2],
    render_bind_groups: [wgpu::BindGroup; 2],
    step_index: usize,
    pub grid_width: u32,
    pub grid_height: u32,
    pub generation: u32,
    // CPU-side mirror for cell toggling without GPU readback
    cells: Vec<u32>,
}

impl Simulation {
    pub async fn new(canvas_id: &str, grid_width: u32, grid_height: u32) -> Self {
        let window = web_sys::window().expect("no window");
        let document = window.document().expect("no document");
        let canvas = document
            .get_element_by_id(canvas_id)
            .expect("no canvas element")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("element is not a canvas");

        let canvas_width = canvas.width();
        let canvas_height = canvas.height();

        // Create wgpu instance targeting WebGPU + WebGL2 fallback
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
            ..Default::default()
        });

        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .expect("failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("failed to get adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                    ..Default::default()
                },
                None, // trace path
            )
            .await
            .expect("failed to get device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: canvas_width,
            height: canvas_height,
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &surface_config);

        // Initialize cell data with random values
        let cell_count = (grid_width * grid_height) as usize;
        let mut cells = vec![0u32; cell_count];
        Self::randomize_cells(&mut cells);

        // Create uniform buffer with grid dimensions
        let grid_data = [grid_width, grid_height];
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("grid uniform"),
            contents: bytemuck::cast_slice(&grid_data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create two cell storage buffers (ping-pong)
        let cell_buffers = [
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("cells A"),
                contents: bytemuck::cast_slice(&cells),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            }),
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("cells B"),
                contents: bytemuck::cast_slice(&vec![0u32; cell_count]),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            }),
        ];

        // --- Compute pipeline ---
        let compute_shader =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("compute shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("compute.wgsl").into()),
            });

        let compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("compute bind group layout"),
                entries: &[
                    // uniform grid
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // cells_in (read-only storage)
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // cells_out (read-write storage)
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let compute_bind_groups = [
            // Step 0: read A, write B
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("compute bind group A->B"),
                layout: &compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: cell_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: cell_buffers[1].as_entire_binding(),
                    },
                ],
            }),
            // Step 1: read B, write A
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("compute bind group B->A"),
                layout: &compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: cell_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: cell_buffers[0].as_entire_binding(),
                    },
                ],
            }),
        ];

        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("compute pipeline layout"),
                bind_group_layouts: &[&compute_bind_group_layout],
                push_constant_ranges: &[],
            });

        let compute_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("compute pipeline"),
                layout: Some(&compute_pipeline_layout),
                module: &compute_shader,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });

        // --- Render pipeline ---
        let render_shader =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("render shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("render.wgsl").into()),
            });

        let render_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("render bind group layout"),
                entries: &[
                    // uniform grid
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // cells (read-only storage)
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let render_bind_groups = [
            // Read from buffer A
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render bind group A"),
                layout: &render_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: cell_buffers[0].as_entire_binding(),
                    },
                ],
            }),
            // Read from buffer B
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render bind group B"),
                layout: &render_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: cell_buffers[1].as_entire_binding(),
                    },
                ],
            }),
        ];

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render pipeline layout"),
                bind_group_layouts: &[&render_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("render pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &render_shader,
                    entry_point: Some("vs"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &render_shader,
                    entry_point: Some("fs"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        Self {
            device,
            queue,
            surface,
            surface_config,
            compute_pipeline,
            render_pipeline,
            cell_buffers,
            uniform_buffer,
            compute_bind_groups,
            render_bind_groups,
            step_index: 0,
            grid_width,
            grid_height,
            generation: 0,
            cells,
        }
    }

    /// Advance one generation and render
    pub fn step(&mut self) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("step encoder"),
            });

        // Compute pass: run Game of Life rules
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("compute pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.compute_pipeline);
            pass.set_bind_group(0, &self.compute_bind_groups[self.step_index], &[]);
            let wg_x = (self.grid_width + 7) / 8;
            let wg_y = (self.grid_height + 7) / 8;
            pass.dispatch_workgroups(wg_x, wg_y, 1);
        }

        // Swap: the output buffer is now the current state
        self.step_index = 1 - self.step_index;
        self.generation += 1;

        // Update CPU-side mirror from the output we just wrote
        // The output buffer index after swap: the buffer that was written to
        // Before swap step_index pointed to the compute bind group (A->B or B->A)
        // After swap, the "current" buffer for rendering is at render_bind_groups[step_index]
        // The written-to buffer index = 1 - old_step_index = step_index after swap
        // We can't easily read back from GPU, so we simulate on CPU too
        self.simulate_cpu_step();

        // Render pass: draw current state
        let output = self.surface.get_current_texture().expect("no surface texture");
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.06,
                            g: 0.06,
                            b: 0.12,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.render_pipeline);
            // Render from the buffer that was just written to
            pass.set_bind_group(0, &self.render_bind_groups[self.step_index], &[]);
            pass.draw(0..6, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    /// Render current state without advancing simulation
    pub fn render(&self) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render encoder"),
            });

        let output = self.surface.get_current_texture().expect("no surface texture");
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.06,
                            g: 0.06,
                            b: 0.12,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.render_pipeline);
            // Render from the current input buffer
            pass.set_bind_group(0, &self.render_bind_groups[self.step_index], &[]);
            pass.draw(0..6, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    /// Simulate one step on CPU to keep the mirror in sync (age-aware)
    fn simulate_cpu_step(&mut self) {
        let w = self.grid_width as i32;
        let h = self.grid_height as i32;
        let old = self.cells.clone();
        for y in 0..h {
            for x in 0..w {
                let mut neighbors = 0u32;
                for dy in -1..=1i32 {
                    for dx in -1..=1i32 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = ((x + dx) % w + w) % w;
                        let ny = ((y + dy) % h + h) % h;
                        if old[(ny * w + nx) as usize] > 0 {
                            neighbors += 1;
                        }
                    }
                }
                let idx = (y * w + x) as usize;
                let age = old[idx];
                let was_alive = age > 0;
                self.cells[idx] = if neighbors == 3 && !was_alive {
                    1 // birth
                } else if was_alive && (neighbors == 2 || neighbors == 3) {
                    (age + 1).min(255) // survive, age up
                } else {
                    0 // death
                };
            }
        }
    }

    /// Upload CPU cells to the current GPU input buffer and render
    fn upload_and_render(&mut self) {
        // The current input buffer is at index matching step_index for render
        // After a step, step_index points to the newly written buffer
        // For direct manipulation, we write to the buffer that render reads from
        let buf_idx = self.step_index;
        self.queue
            .write_buffer(&self.cell_buffers[buf_idx], 0, bytemuck::cast_slice(&self.cells));
        self.render();
    }

    pub fn toggle_cell(&mut self, x: u32, y: u32) {
        if x < self.grid_width && y < self.grid_height {
            let idx = (y * self.grid_width + x) as usize;
            self.cells[idx] = if self.cells[idx] > 0 { 0 } else { 1 };
            self.upload_and_render();
        }
    }

    pub fn set_cell(&mut self, x: u32, y: u32, alive: bool) {
        if x < self.grid_width && y < self.grid_height {
            let idx = (y * self.grid_width + x) as usize;
            self.cells[idx] = if alive { 1 } else { 0 };
            self.upload_and_render();
        }
    }

    pub fn reset_random(&mut self) {
        Self::randomize_cells(&mut self.cells);
        self.generation = 0;
        self.upload_and_render();
    }

    pub fn clear(&mut self) {
        self.cells.fill(0);
        self.generation = 0;
        self.upload_and_render();
    }

    pub fn population(&self) -> u32 {
        self.cells.iter().filter(|&&c| c > 0).count() as u32
    }

    pub fn resize_surface(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }

    fn randomize_cells(cells: &mut [u32]) {
        // Simple LCG random since we're in WASM (no std::rand)
        let seed = js_sys::Math::random();
        for (i, cell) in cells.iter_mut().enumerate() {
            // Use Math.random() for each cell
            let r = js_sys::Math::random();
            *cell = if r < 0.25 { 1 } else { 0 };
            let _ = (seed, i); // suppress unused warning
        }
    }
}
