use crate::config::{BuiltinTheme, WallpaperSource};
use bytemuck::{Pod, Zeroable};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

const BUILTIN_SHADER: &str = include_str!("shader.wgsl");

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    resolution: [f32; 2],
    time: f32,
    theme_id: u32,
    mode: u32, // 0 = theme, 1 = custom image
    image_aspect: f32,
    _pad0: f32,
    _pad1: f32,
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline_layout: wgpu::PipelineLayout,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    uniform_buf: wgpu::Buffer,
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    uniforms: Uniforms,
    using_custom_shader: bool,
}

impl Renderer {
    pub fn new(window: Arc<Window>, source: &WallpaperSource) -> Result<Self, String> {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance
            .create_surface(window)
            .map_err(|e| format!("Failed to create GPU surface: {e}"))?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| "No suitable GPU adapter found".to_string())?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("umbra-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
            },
            None,
        ))
        .map_err(|e| format!("Failed to request GPU device: {e}"))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let uniforms = Uniforms {
            resolution: [width as f32, height as f32],
            time: 0.0,
            theme_id: BuiltinTheme::ObsidianDrift.shader_index(),
            mode: 0,
            image_aspect: 16.0 / 9.0,
            _pad0: 0.0,
            _pad1: 0.0,
        };

        let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("umbra-uniforms"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("umbra-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("umbra-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("umbra-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = Self::build_pipeline(&device, &pipeline_layout, format, BUILTIN_SHADER)
            .ok_or_else(|| "Failed to compile built-in WGSL shader".to_string())?;

        let dummy_view = Self::create_1x1_texture(&device, &queue);
        let bind_group =
            Self::create_bind_group(&device, &bind_group_layout, &uniform_buf, &dummy_view, &sampler);

        let mut renderer = Self {
            surface,
            device,
            queue,
            config,
            pipeline_layout,
            bind_group_layout,
            pipeline,
            uniform_buf,
            sampler,
            bind_group,
            uniforms,
            using_custom_shader: false,
        };

        renderer.set_source(source);
        Ok(renderer)
    }

    fn build_pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        format: wgpu::TextureFormat,
        wgsl_src: &str,
    ) -> Option<wgpu::RenderPipeline> {
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("umbra-shader"),
            source: wgpu::ShaderSource::Wgsl(wgsl_src.into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("umbra-pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        if pollster::block_on(device.pop_error_scope()).is_some() {
            return None;
        }
        Some(pipeline)
    }

    fn create_1x1_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
        let size = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("umbra-placeholder-tex"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[8, 9, 12, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            size,
        );
        tex.create_view(&wgpu::TextureViewDescriptor::default())
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform_buf: &wgpu::Buffer,
        tex_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("umbra-bg"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(tex_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    fn restore_builtin_pipeline(&mut self) {
        if self.using_custom_shader {
            if let Some(p) = Self::build_pipeline(
                &self.device,
                &self.pipeline_layout,
                self.config.format,
                BUILTIN_SHADER,
            ) {
                self.pipeline = p;
                self.using_custom_shader = false;
            }
        }
    }

    pub fn set_source(&mut self, source: &WallpaperSource) {
        match source {
            WallpaperSource::Theme { theme } => {
                self.restore_builtin_pipeline();
                self.uniforms.mode = 0;
                self.uniforms.theme_id = theme.shader_index();
            }
            WallpaperSource::Image { path } => {
                self.restore_builtin_pipeline();
                if let Some((view, aspect)) = self.load_image_texture(path) {
                    self.bind_group = Self::create_bind_group(
                        &self.device,
                        &self.bind_group_layout,
                        &self.uniform_buf,
                        &view,
                        &self.sampler,
                    );
                    self.uniforms.mode = 1;
                    self.uniforms.image_aspect = aspect;
                } else {
                    // Fallback to Obsidian Drift if image fails to load
                    self.uniforms.mode = 0;
                    self.uniforms.theme_id = BuiltinTheme::ObsidianDrift.shader_index();
                }
            }
            WallpaperSource::Shader { path } => {
                if let Ok(src) = fs::read_to_string(path) {
                    if let Some(p) = Self::build_pipeline(
                        &self.device,
                        &self.pipeline_layout,
                        self.config.format,
                        &src,
                    ) {
                        self.pipeline = p;
                        self.using_custom_shader = true;
                        self.uniforms.mode = 0;
                        return;
                    }
                }
                self.restore_builtin_pipeline();
                self.uniforms.mode = 0;
                self.uniforms.theme_id = BuiltinTheme::ObsidianDrift.shader_index();
            }
        }
    }

    fn load_image_texture(&self, path: &Path) -> Option<(wgpu::TextureView, f32)> {
        let img = image::open(path).ok()?.to_rgba8();
        let (w, h) = img.dimensions();
        if w == 0 || h == 0 {
            return None;
        }
        let max_dim = self.device.limits().max_texture_dimension_2d;
        let (w, h, data) = if w > max_dim || h > max_dim {
            let resized = image::imageops::resize(
                &img,
                w.min(max_dim),
                h.min(max_dim),
                image::imageops::FilterType::Lanczos3,
            );
            let (rw, rh) = resized.dimensions();
            (rw, rh, resized.into_raw())
        } else {
            (w, h, img.into_raw())
        };

        let size = wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        };
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("umbra-wallpaper-tex"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * w),
                rows_per_image: Some(h),
            },
            size,
        );

        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        Some((view, w as f32 / h as f32))
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.uniforms.resolution = [width as f32, height as f32];
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render(&mut self, elapsed_secs: f32) {
        self.uniforms.time = elapsed_secs;
        self.queue
            .write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&self.uniforms));

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Err(_) => return,
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("umbra-encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("umbra-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }
}
