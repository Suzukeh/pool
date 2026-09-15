//! wgpu レンダラ本体。単一 WGSL パイプラインの多パス合成。
//!
//! 描画順：レイヤー index 0 → 末尾（下が手前）。各オブジェクトは
//! スクラッチに描いて効果をかけてから over 合成する。

use crate::text;
use ab_glyph::FontRef;
use pool_timeline_model::{Frame, ObjectKind, Rgba, Scene, ShapeKind, TimelineObject};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
pub enum RenderError {
    NoAdapter,
    Device(String),
    Png(String),
    Readback(String),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAdapter => write!(f, "no usable GPU adapter (tried Vulkan, GL)"),
            Self::Device(e) => write!(f, "device error: {e}"),
            Self::Png(e) => write!(f, "png encode error: {e}"),
            Self::Readback(e) => write!(f, "readback error: {e}"),
        }
    }
}

impl std::error::Error for RenderError {}

const MODE_SHAPE: u32 = 0;
const MODE_BLIT: u32 = 1;
const MODE_BRIGHT: u32 = 2;
const MODE_BLUR: u32 = 3;
const MODE_OVER: u32 = 4;

const SHAPE_RECT: u32 = 0;
const SHAPE_ELLIPSE: u32 = 1;

/// WGSL `U` と一致させること（96 bytes）。
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct U {
    resolution: [f32; 2],
    mode: u32,
    shape: u32,
    color: [f32; 4],
    opacity: f32,
    _pad0: f32,
    center: [f32; 2],
    size: [f32; 2],
    brightness: f32,
    blur_radius: f32,
    blur_dir: [f32; 2],
    _pad1: [f32; 2],
    uv_rect: [f32; 4],
}

impl U {
    fn base(w: f32, h: f32, mode: u32) -> Self {
        Self {
            resolution: [w, h],
            mode,
            shape: 0,
            color: [0.0, 0.0, 0.0, 0.0],
            opacity: 1.0,
            _pad0: 0.0,
            center: [0.0, 0.0],
            size: [1.0, 1.0],
            brightness: 0.0,
            blur_radius: 0.0,
            blur_dir: [0.0, 0.0],
            _pad1: [0.0, 0.0],
            uv_rect: [0.0, 0.0, 1.0, 1.0],
        }
    }
}

fn hash01(n: f64) -> f64 {
    let s = (n * 12.9898).sin() * 43758.5453;
    s - s.floor()
}

fn vnoise(t: f64) -> f64 {
    let i = t.floor();
    let f = t - i;
    let u = f * f * (3.0 - 2.0 * f);
    hash01(i) * (1.0 - u) + hash01(i + 1.0) * u
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    pipeline_blend: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    dummy: wgpu::TextureView,
    font: FontRef<'static>,
    text_cache: HashMap<(String, u32), (wgpu::Texture, u32, u32)>,
}

fn adapter_for(backends: wgpu::Backends) -> Option<wgpu::Adapter> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends,
        ..Default::default()
    });
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .ok()
}

impl Renderer {
    pub fn new() -> Result<Self, RenderError> {
        let adapter = [
            wgpu::Backends::VULKAN,
            wgpu::Backends::GL,
            wgpu::Backends::all(),
        ]
        .into_iter()
        .find_map(adapter_for)
        .ok_or(RenderError::NoAdapter)?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("pool"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
        }))
        .map_err(|e| RenderError::Device(e.to_string()))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pool composite"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pool bg"),
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
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pool layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pool composite"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        // 最終合成用：straight-alpha over をハードブレンドで行う。
        let pipeline_blend = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pool composite blend"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pool uniform"),
            size: std::mem::size_of::<U>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dummy = Self::make_texture(&device, 1, 1);
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let font = FontRef::try_from_slice(include_bytes!("../assets/DejaVuSans.ttf"))
            .map_err(|e| RenderError::Device(format!("font: {e:?}")))?;

        Ok(Self {
            device,
            queue,
            pipeline,
            pipeline_blend,
            bgl,
            sampler,
            uniform,
            dummy: dummy.1,
            font,
            text_cache: HashMap::new(),
        })
    }

    fn make_texture(device: &wgpu::Device, w: u32, h: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("pool rt"),
            size: wgpu::Extent3d {
                width: w.max(1),
                height: h.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        (tex, view)
    }

    fn bind(&self, src: &wgpu::TextureView, extra: &wgpu::TextureView) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pool pass bg"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(src),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(extra),
                },
            ],
        })
    }

    /// 1 パス実行。`clear` 指定時はクリアのみ（描画なし）。
    /// `blend` 指定時はハードブレンドパイプライン（最終合成用）。
    fn run_pass(
        &self,
        dst: &wgpu::TextureView,
        src: &wgpu::TextureView,
        extra: &wgpu::TextureView,
        u: &U,
        clear: Option<wgpu::Color>,
        blend: bool,
    ) {
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(u));
        let bg = self.bind(src, extra);
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("pool pass"),
            });
        {
            let ops = match clear {
                Some(c) => wgpu::Operations {
                    load: wgpu::LoadOp::Clear(c),
                    store: wgpu::StoreOp::Store,
                },
                None => wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            };
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pool rp"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: dst,
                    depth_slice: None,
                    resolve_target: None,
                    ops,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if clear.is_none() {
                rp.set_pipeline(if blend {
                    &self.pipeline_blend
                } else {
                    &self.pipeline
                });
                rp.set_bind_group(0, &bg, &[]);
                rp.draw(0..3, 0..1);
            }
        }
        self.queue.submit(Some(enc.finish()));
    }

    fn upload_rgba(&self, w: u32, h: u32, rgba: &[u8]) -> (wgpu::Texture, wgpu::TextureView) {
        let (tex, view) = Self::make_texture(&self.device, w, h);
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        (tex, view)
    }

    fn text_view(&mut self, body: &str, size_px: u32) -> (wgpu::TextureView, u32, u32) {
        let key = (body.to_string(), size_px);
        if !self.text_cache.contains_key(&key) {
            let bmp = text::rasterize(&self.font, body, size_px as f32);
            let (tex, _) = self.upload_rgba(bmp.width, bmp.height, &bmp.rgba);
            self.text_cache
                .insert(key.clone(), (tex, bmp.width, bmp.height));
        }
        let (tex, w, h) = &self.text_cache[&key];
        (
            tex.create_view(&wgpu::TextureViewDescriptor::default()),
            *w,
            *h,
        )
    }

    fn rgba(c: Rgba) -> [f32; 4] {
        [c.r, c.g, c.b, c.a]
    }

    /// シーンを RGBA8 でレンダリングする。
    pub fn render_scene(
        &mut self,
        scene: &Scene,
        frame: Frame,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, RenderError> {
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        let (comp_tex, comp_view) = Self::make_texture(&self.device, width.max(1), height.max(1));
        let (_scratch_tex, scratch_view) =
            Self::make_texture(&self.device, width.max(1), height.max(1));
        let (_scratch2_tex, scratch2_view) =
            Self::make_texture(&self.device, width.max(1), height.max(1));
        let (_tmp_tex, tmp_view) = Self::make_texture(&self.device, width.max(1), height.max(1));

        let bg = scene.bg_color;
        let u = U::base(w, h, MODE_SHAPE);
        self.run_pass(
            &comp_view,
            &comp_view,
            &self.dummy,
            &u,
            Some(wgpu::Color {
                r: bg.r as f64,
                g: bg.g as f64,
                b: bg.b as f64,
                a: bg.a as f64,
            }),
            false,
        );

        for layer in &scene.layers {
            if !layer.visible {
                continue;
            }
            for o in &layer.objects {
                // フィルタは後段で一括適用
                if matches!(o.kind, ObjectKind::Filter { .. }) {
                    continue;
                }
                if o.contains(frame) {
                    self.draw_object(
                        o,
                        scene,
                        frame,
                        w,
                        h,
                        &comp_view,
                        &scratch_view,
                        &scratch2_view,
                        &tmp_view,
                    )?;
                }
            }
        }

        // フィルタオブジェクト（区間内）を全体に適用
        for layer in &scene.layers {
            for o in &layer.objects {
                if o.contains(frame) {
                    if let ObjectKind::Filter { .. } = &o.kind {
                        self.apply_filter(o, frame, &comp_view, &tmp_view, w, h);
                    }
                }
            }
        }

        self.readback(&comp_tex, width.max(1), height.max(1))
    }

    fn apply_filter(
        &self,
        o: &TimelineObject,
        frame: Frame,
        comp: &wgpu::TextureView,
        tmp: &wgpu::TextureView,
        w: f32,
        h: f32,
    ) {
        if let ObjectKind::Filter { effect } = &o.kind {
            match effect.as_str() {
                "brightness" => {
                    let b = o.eval_number("brightness", frame, 0.0) as f32;
                    if b != 0.0 {
                        let mut u = U::base(w, h, MODE_BRIGHT);
                        u.brightness = b;
                        self.run_pass(tmp, comp, &self.dummy, &u, None, false);
                        // tmp → comp へ上書き相当（全面 over、不透明なので実質コピー）
                        self.blit_over(comp, tmp, w, h);
                    }
                }
                "blur" => {
                    let r = o.eval_number("blur", frame, 0.0) as f32;
                    if r > 0.0 {
                        self.blur_into(comp, tmp, w, h, r);
                    }
                }
                _ => {}
            }
        }
    }

    /// src 全面を dst に over 合成（ハードブレンド）。
    fn blit_over(&self, dst: &wgpu::TextureView, src: &wgpu::TextureView, w: f32, h: f32) {
        let u = U::base(w, h, MODE_OVER);
        self.run_pass(dst, &self.dummy, src, &u, None, true);
    }

    fn blur_into(
        &self,
        comp: &wgpu::TextureView,
        tmp: &wgpu::TextureView,
        w: f32,
        h: f32,
        radius: f32,
    ) {
        let mut uh = U::base(w, h, MODE_BLUR);
        uh.blur_radius = radius;
        uh.blur_dir = [1.0, 0.0];
        self.run_pass(tmp, comp, &self.dummy, &uh, None, false);
        let mut uv = U::base(w, h, MODE_BLUR);
        uv.blur_radius = radius;
        uv.blur_dir = [0.0, 1.0];
        self.run_pass(comp, tmp, &self.dummy, &uv, None, false);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_object(
        &mut self,
        o: &TimelineObject,
        scene: &Scene,
        frame: Frame,
        w: f32,
        h: f32,
        comp: &wgpu::TextureView,
        scratch: &wgpu::TextureView,
        scratch2: &wgpu::TextureView,
        tmp: &wgpu::TextureView,
    ) -> Result<(), RenderError> {
        match &o.kind {
            ObjectKind::Filter { .. } => return Ok(()), // 後段で一括適用
            ObjectKind::Audio { .. } => return Ok(()),
            ObjectKind::GroupControl { .. } | ObjectKind::CameraControl { .. } => return Ok(()), // M3
            ObjectKind::Duplicator { source } => {
                return self
                    .draw_duplicator(o, source, scene, frame, w, h, comp, scratch, scratch2, tmp);
            }
            _ => {}
        }
        let Some(p) = self.resolve_paint(o, frame, w, h, 0.0, 0.0, 0) else {
            return Ok(());
        };
        // スクラッチを透明クリア（src=dummy で自己参照を避ける）
        let clear = U::base(w, h, MODE_SHAPE);
        self.run_pass(
            scratch,
            &self.dummy,
            &self.dummy,
            &clear,
            Some(wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }),
            false,
        );
        self.paint(&p, w, h, scratch, &self.dummy);
        self.finish_drawable(o, frame, w, h, comp, scratch, tmp);
        Ok(())
    }
}

/// 描画内容（GPU リソース解決済み・所有）。
enum Paint {
    Shape {
        shape: u32,
        color: [f32; 4],
        opacity: f32,
        cx: f32,
        cy: f32,
        sw: f32,
        sh: f32,
    },
    Blit {
        view: wgpu::TextureView,
        tw: f32,
        th: f32,
        color: [f32; 4],
        opacity: f32,
        cx: f32,
        cy: f32,
        scale: f32,
    },
}

impl Renderer {
    fn wobble(o: &TimelineObject, frame: Frame, idx: i64) -> (f32, f32) {
        let amp = o.eval_number("wobble_amp", frame, 0.0);
        if amp == 0.0 {
            return (0.0, 0.0);
        }
        let speed = o.eval_number("wobble_speed", frame, 1.0);
        let t = frame as f64 * speed + idx as f64 * 0.37;
        (
            (amp * (vnoise(t) - 0.5) * 2.0) as f32,
            (amp * (vnoise(t + 7.31) - 0.5) * 2.0) as f32,
        )
    }

    /// オブジェクト→描画内容。中心オフセット (dx,dy)・複製位相付き。
    #[allow(clippy::too_many_arguments)]
    fn resolve_paint(
        &mut self,
        o: &TimelineObject,
        frame: Frame,
        w: f32,
        h: f32,
        dx: f32,
        dy: f32,
        idx: i64,
    ) -> Option<Paint> {
        let (wx, wy) = Self::wobble(o, frame, idx);
        let cx = o.eval_number("x", frame, (w / 2.0) as f64) as f32 + dx + wx;
        let cy = o.eval_number("y", frame, (h / 2.0) as f64) as f32 + dy + wy;
        let scale = o.eval_number("scale", frame, 1.0) as f32;
        let opacity = o.eval_number("opacity", frame, 1.0) as f32;
        match &o.kind {
            ObjectKind::Shape { shape } => {
                let (dw, dh) = match shape {
                    ShapeKind::Rectangle => (320.0, 180.0),
                    _ => (200.0, 200.0),
                };
                Some(Paint::Shape {
                    shape: match shape {
                        ShapeKind::Rectangle => SHAPE_RECT,
                        _ => SHAPE_ELLIPSE,
                    },
                    color: Self::rgba(o.eval_color("fill", frame, Rgba::WHITE)),
                    opacity,
                    cx,
                    cy,
                    sw: o.eval_number("w", frame, dw) as f32 * scale,
                    sh: o.eval_number("h", frame, dh) as f32 * scale,
                })
            }
            ObjectKind::Text { body } => {
                let size = o.eval_number("size", frame, 40.0) as u32;
                let (view, tw, th) = self.text_view(body, size.max(8));
                Some(Paint::Blit {
                    view,
                    tw: tw as f32,
                    th: th as f32,
                    color: Self::rgba(o.eval_color("fill", frame, Rgba::WHITE)),
                    opacity,
                    cx,
                    cy,
                    scale,
                })
            }
            ObjectKind::Video { .. } | ObjectKind::Image { .. } => {
                // M2 スタブ：スレート色矩形（デコードは M5）
                Some(Paint::Shape {
                    shape: SHAPE_RECT,
                    color: [0.23, 0.27, 0.34, 1.0],
                    opacity,
                    cx,
                    cy,
                    sw: o.eval_number("w", frame, 640.0) as f32 * scale,
                    sh: o.eval_number("h", frame, 360.0) as f32 * scale,
                })
            }
            _ => None,
        }
    }

    /// 描画内容を dst に over（src は下地）。同一テクスチャ禁止。
    fn paint(&self, p: &Paint, w: f32, h: f32, dst: &wgpu::TextureView, src: &wgpu::TextureView) {
        match p {
            Paint::Shape {
                shape,
                color,
                opacity,
                cx,
                cy,
                sw,
                sh,
            } => {
                let mut u = U::base(w, h, MODE_SHAPE);
                u.shape = *shape;
                u.color = *color;
                u.opacity = *opacity;
                u.center = [*cx, *cy];
                u.size = [*sw, *sh];
                self.run_pass(dst, src, &self.dummy, &u, None, false);
            }
            Paint::Blit {
                view,
                tw,
                th,
                color,
                opacity,
                cx,
                cy,
                scale,
            } => {
                let mut u = U::base(w, h, MODE_BLIT);
                u.color = *color;
                u.opacity = *opacity;
                u.center = [*cx, *cy];
                u.size = [tw * scale, th * scale];
                self.run_pass(dst, src, view, &u, None, false);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_duplicator(
        &mut self,
        o: &TimelineObject,
        source: &str,
        scene: &Scene,
        frame: Frame,
        w: f32,
        h: f32,
        comp: &wgpu::TextureView,
        scratch: &wgpu::TextureView,
        scratch2: &wgpu::TextureView,
        tmp: &wgpu::TextureView,
    ) -> Result<(), RenderError> {
        let src = scene
            .layers
            .iter()
            .flat_map(|l| &l.objects)
            .find(|c| c.id == *source);
        let src = match src {
            Some(s) => s.clone(),
            None => return Ok(()),
        };
        let cols = o.eval_int("cols", frame, 3).max(1) as usize;
        let rows = o.eval_int("rows", frame, 3).max(1) as usize;
        let sx = o.eval_number("spacing_x", frame, 80.0) as f32;
        let sy = o.eval_number("spacing_y", frame, 80.0) as f32;
        let stagger = o.eval_int("stagger", frame, 0);
        let cx = o.eval_number("x", frame, (w / 2.0) as f64) as f32;
        let cy = o.eval_number("y", frame, (h / 2.0) as f64) as f32;
        let base_cx = src.eval_number("x", frame, (w / 2.0) as f64) as f32;
        let base_cy = src.eval_number("y", frame, (h / 2.0) as f64) as f32;

        let clear = U::base(w, h, MODE_SHAPE);
        let transparent = Some(wgpu::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        });
        self.run_pass(
            scratch,
            &self.dummy,
            &self.dummy,
            &clear,
            transparent,
            false,
        );
        self.run_pass(
            scratch2,
            &self.dummy,
            &self.dummy,
            &clear,
            transparent,
            false,
        );
        // ping-pong 蓄積（同一テクスチャ読み書きの禁止を避ける）
        let mut read_is_scratch = true;
        let mut drawn = 0u32;
        for r in 0..rows {
            for c in 0..cols {
                let idx = (r * cols + c) as i64;
                if stagger != 0 {
                    let lf = frame - idx * stagger;
                    if lf < src.start_frame || lf >= src.end_frame {
                        continue;
                    }
                }
                let ox = (c as f32 - (cols as f32 - 1.0) / 2.0) * sx;
                let oy = (r as f32 - (rows as f32 - 1.0) / 2.0) * sy;
                let dx = cx + ox - base_cx;
                let dy = cy + oy - base_cy;
                let Some(p) = self.resolve_paint(&src, frame, w, h, dx, dy, idx) else {
                    continue;
                };
                let (rd, wr) = if read_is_scratch {
                    (scratch, scratch2)
                } else {
                    (scratch2, scratch)
                };
                self.paint(&p, w, h, wr, rd);
                read_is_scratch = !read_is_scratch;
                drawn += 1;
            }
        }
        // 最終内容を scratch に集約
        if drawn % 2 == 1 {
            self.blit_over(scratch, scratch2, w, h);
        }
        self.finish_drawable(o, frame, w, h, comp, scratch, tmp);
        Ok(())
    }

    /// scratch 上の描画に効果をかけて comp に over。
    #[allow(clippy::too_many_arguments)]
    fn finish_drawable(
        &self,
        o: &TimelineObject,
        frame: Frame,
        w: f32,
        h: f32,
        comp: &wgpu::TextureView,
        scratch: &wgpu::TextureView,
        tmp: &wgpu::TextureView,
    ) {
        let b = o.eval_number("brightness", frame, 0.0) as f32;
        if b != 0.0 {
            let mut u = U::base(w, h, MODE_BRIGHT);
            u.brightness = b;
            self.run_pass(tmp, scratch, &self.dummy, &u, None, false);
            self.blit_over(scratch, tmp, w, h);
        }
        let r = o.eval_number("blur", frame, 0.0) as f32;
        if r > 0.0 {
            self.blur_into(scratch, tmp, w, h, r);
        }
        self.blit_over(comp, scratch, w, h);
    }

    fn readback(
        &self,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, RenderError> {
        let padded = (width * 4).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pool readback"),
            size: (padded * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("pool copy"),
            });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(enc.finish()));

        let slice = buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait())
            .map_err(|e| RenderError::Readback(format!("poll: {e:?}")))?;
        rx.recv()
            .map_err(|e| RenderError::Readback(format!("recv: {e}")))?
            .map_err(|e| RenderError::Readback(format!("map: {e:?}")))?;
        let mapped = slice.get_mapped_range();
        // premultiplied → straight 変換（表示・PNG 用）
        let mut out = Vec::with_capacity((width * height * 4) as usize);
        for row in 0..height as usize {
            let start = row * padded as usize;
            for x in 0..width as usize {
                let o = start + x * 4;
                let a = mapped[o + 3] as u32;
                if a == 0 {
                    out.extend_from_slice(&[0, 0, 0, 0]);
                } else {
                    let un = |c: u8| ((c as u32 * 255 + a / 2) / a).min(255) as u8;
                    out.extend_from_slice(&[
                        un(mapped[o]),
                        un(mapped[o + 1]),
                        un(mapped[o + 2]),
                        a as u8,
                    ]);
                }
            }
        }
        drop(mapped);
        buf.unmap();
        Ok(out)
    }

    /// PNG バイト列でレンダリング（プレビュー配信用）。
    pub fn render_png(
        &mut self,
        scene: &Scene,
        frame: Frame,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, RenderError> {
        let rgba = self.render_scene(scene, frame, width, height)?;
        let mut out = Vec::new();
        let enc = image::codecs::png::PngEncoder::new(&mut out);
        use image::ImageEncoder;
        enc.write_image(
            &rgba,
            width.max(1),
            height.max(1),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| RenderError::Png(e.to_string()))?;
        Ok(out)
    }
}
