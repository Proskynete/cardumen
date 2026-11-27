//! wgpu renderer: one instanced quad pipeline (backgrounds, glyphs, decorations, cursor).
mod atlas;

use std::sync::Arc;

use alacritty_terminal::event::EventListener;
use alacritty_terminal::term::Term;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{CursorShape, NamedColor};
use winit::window::Window;

use crate::font::{Face, Fonts};
use crate::theme::{self, Rgb};
use atlas::{Atlas, AtlasKey, Entry, GlyphKey};

#[repr(C)]
#[derive(Clone, Copy)]
struct Quad {
    pos: [f32; 2],
    size: [f32; 2],
    uv: [f32; 4],
    color: [f32; 4],
    kind: u32,
    _pad: [u32; 3],
}
// SAFETY: repr(C), only f32/u32 fields, explicit padding, no implicit padding bytes.
unsafe impl bytemuck::Zeroable for Quad {}
unsafe impl bytemuck::Pod for Quad {}

const SOLID: u32 = 0;
const MASK: u32 = 1;
const COLOR: u32 = 2;
const ROUND: u32 = 3;

pub struct CellDraw {
    pub col: usize,
    pub line: usize,
    pub c: char,
    pub fg: Rgb,
    pub bg: Rgb,
    pub flags: Flags,
}

pub struct CursorDraw {
    pub col: usize,
    pub line: usize,
    pub shape: CursorShape,
    pub color: Rgb,
}

/// Everything needed to draw one frame, copied out while the term lock is held.
pub struct Frame {
    pub cells: Vec<CellDraw>,
    pub cursor: Option<CursorDraw>,
}

impl Frame {
    pub fn capture<L: EventListener>(term: &Term<L>, focused: bool, cursor_on: bool) -> Self {
        let content = term.renderable_content();
        let offset = content.display_offset as i32;
        let cursor_color = content.colors[NamedColor::Cursor].map_or(theme::ACCENT, |c| [c.r, c.g, c.b]);
        let mut cursor = None;
        if content.cursor.shape != CursorShape::Hidden && cursor_on {
            let line = content.cursor.point.line.0 + offset;
            if line >= 0 {
                let shape = if focused { content.cursor.shape } else { CursorShape::HollowBlock };
                cursor = Some(CursorDraw { col: content.cursor.point.column.0, line: line as usize, shape, color: cursor_color });
            }
        }
        let block_at = cursor.as_ref().filter(|c| c.shape == CursorShape::Block).map(|c| (c.col, c.line));
        let mut cells = Vec::new();
        for item in content.display_iter {
            let cell = item.cell;
            if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                continue;
            }
            let line = (item.point.line.0 + offset) as usize;
            let col = item.point.column.0;
            let mut fg = theme::resolve(cell.fg, content.colors);
            let mut bg = theme::resolve(cell.bg, content.colors);
            if cell.flags.contains(Flags::DIM) {
                fg = theme::dim(fg);
            }
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            if content.selection.is_some_and(|s| s.contains(item.point)) {
                bg = theme::SELECTION;
            }
            if cell.flags.contains(Flags::HIDDEN) {
                fg = bg;
            }
            if block_at == Some((col, line)) {
                bg = cursor_color;
                fg = theme::BACKGROUND;
            }
            let decorated = cell.flags.intersects(Flags::ALL_UNDERLINES | Flags::STRIKEOUT);
            if cell.c != ' ' || bg != theme::BACKGROUND || decorated {
                cells.push(CellDraw { col, line, c: cell.c, fg, bg, flags: cell.flags });
            }
        }
        Self { cells, cursor }
    }

    /// Draws IME composition text (e.g. a pending `´`) underlined at the cursor.
    /// shortcut: one cell per char, wide (CJK) preedit overlaps; upgrade with unicode-width if needed.
    pub fn overlay_preedit(&mut self, text: &str) {
        let Some(cursor) = self.cursor.as_mut() else { return };
        let (col, line) = (cursor.col, cursor.line);
        let n = text.chars().count();
        self.cells.retain(|c| c.line != line || c.col < col || c.col >= col + n);
        for (i, c) in text.chars().enumerate() {
            let (fg, bg, flags) = (theme::FOREGROUND, theme::BACKGROUND, Flags::UNDERLINE);
            self.cells.push(CellDraw { col: col + i, line, c, fg, bg, flags });
        }
        cursor.col += n;
    }
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniforms: wgpu::Buffer,
    atlas_texture: wgpu::Texture,
    instances: wgpu::Buffer,
    atlas: Atlas,
    quads: Vec<Quad>,
    atlas_reset: bool,
}

fn solid(x: f32, y: f32, w: f32, h: f32, c: Rgb) -> Quad {
    Quad { pos: [x, y], size: [w, h], uv: [0.0; 4], color: rgba(c), kind: SOLID, _pad: [0; 3] }
}

/// Window decoration drawn behind the panes: tab bar, sidebar, panel, pane cards.
pub enum Chrome {
    Rect { rect: crate::layout::Rect, color: Rgb },
    Round { rect: crate::layout::Rect, radius: f32, color: Rgb },
    /// One line of interface text; `y` is the top of the line box (px × 1.3 tall).
    Label { x: f32, y: f32, text: String, face: Face, px: f32, bold: bool, color: Rgb, max_w: f32 },
}

fn rgba(c: Rgb) -> [f32; 4] {
    [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0]
}

impl Renderer {
    pub fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(desc);
        let surface = instance.create_surface(window).expect("create surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .expect("no GPU adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("cardumen"),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        }))
        .expect("no GPU device");

        let caps = surface.get_capabilities(&adapter);
        // Non-sRGB target: colours are already sRGB bytes and blend like other macOS terminals.
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            color_space: Default::default(),
        };
        surface.configure(&device, &config);

        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph atlas"),
            size: wgpu::Extent3d { width: atlas::SIZE, height: atlas::SIZE, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default()); // nearest: glyphs are pixel-aligned
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
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
            ],
        });
        let view = atlas_texture.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quads"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Quad>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4, 3 => Float32x4, 4 => Uint32],
                })],
            },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let instances = Self::instance_buffer(&device, 4096);
        Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            bind_group,
            uniforms,
            atlas_texture,
            instances,
            atlas: Atlas::default(),
            quads: Vec::new(),
            atlas_reset: false,
        }
    }

    fn instance_buffer(device: &wgpu::Device, quads: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instances"),
            size: (quads * std::mem::size_of::<Quad>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
    }

    /// Drop every cached glyph (font or scale changed).
    pub fn clear_glyphs(&mut self) {
        self.atlas.clear();
    }

    fn glyph(&mut self, fonts: &mut Fonts, key: AtlasKey) -> Option<Entry> {
        if let Some(entry) = self.atlas.map.get(&key) {
            return *entry;
        }
        let image = match key {
            AtlasKey::Cell(k) => fonts.rasterize(k.c, k.bold, k.italic),
            AtlasKey::Run(k) => fonts.raster_key(k),
        };
        let entry = image.and_then(|g| {
            let (x, y) = self.atlas.alloc(g.width, g.height).or_else(|| {
                self.atlas.clear();
                self.atlas_reset = true;
                self.atlas.alloc(g.width, g.height)
            })?;
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x, y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &g.rgba,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * g.width), rows_per_image: Some(g.height) },
                wgpu::Extent3d { width: g.width, height: g.height, depth_or_array_layers: 1 },
            );
            Some(Entry { x, y, w: g.width, h: g.height, left: g.left, top: g.top, color: g.color })
        });
        self.atlas.map.insert(key, entry);
        entry
    }

    /// Queue one terminal frame whose grid starts at `pad` (physical px).
    fn push_frame(&mut self, frame: &Frame, fonts: &mut Fonts, pad: (f32, f32)) {
        let (cw, ch) = (fonts.cell_w, fonts.cell_h);
        let origin = |col: usize, line: usize| (pad.0 + col as f32 * cw, pad.1 + line as f32 * ch);
        let thickness = (fonts.px / 14.0).round().max(1.0);

        for cell in &frame.cells {
            if cell.bg != theme::BACKGROUND {
                let (x, y) = origin(cell.col, cell.line);
                let w = if cell.flags.contains(Flags::WIDE_CHAR) { 2.0 * cw } else { cw };
                self.quads.push(solid(x, y, w, ch, cell.bg));
            }
        }
        for cell in &frame.cells {
            let (x, y) = origin(cell.col, cell.line);
            // Block elements are drawn as exact rects: font glyphs leave seams between rows.
            let half = (ch / 2.0).round();
            match cell.c {
                '▀' => self.quads.push(solid(x, y, cw, half, cell.fg)),
                '▄' => self.quads.push(solid(x, y + half, cw, ch - half, cell.fg)),
                '█' => self.quads.push(solid(x, y, cw, ch, cell.fg)),
                _ => {}
            }
            if !matches!(cell.c, ' ' | '\t' | '▀' | '▄' | '█') {
                let key = AtlasKey::Cell(GlyphKey {
                    c: cell.c,
                    bold: cell.flags.contains(Flags::BOLD),
                    italic: cell.flags.contains(Flags::ITALIC),
                });
                if let Some(e) = self.glyph(fonts, key) {
                    self.quads.push(Quad {
                        pos: [x + e.left as f32, y + fonts.baseline - e.top as f32],
                        size: [e.w as f32, e.h as f32],
                        uv: [e.x as f32, e.y as f32, e.w as f32, e.h as f32],
                        color: rgba(cell.fg),
                        kind: if e.color { COLOR } else { MASK },
                        _pad: [0; 3],
                    });
                }
            }
            if cell.flags.intersects(Flags::ALL_UNDERLINES) {
                self.quads.push(solid(x, y + fonts.baseline + thickness, cw, thickness, cell.fg));
            }
            if cell.flags.contains(Flags::STRIKEOUT) {
                self.quads.push(solid(x, y + (ch / 2.0).round(), cw, thickness, cell.fg));
            }
        }
        if let Some(c) = &frame.cursor {
            let (x, y) = origin(c.col, c.line);
            let t = thickness.max(2.0);
            match c.shape {
                CursorShape::Beam => self.quads.push(solid(x, y, (fonts.px / 7.0).round().max(2.0), ch, c.color)),
                CursorShape::Underline => self.quads.push(solid(x, y + ch - t, cw, t, c.color)),
                CursorShape::HollowBlock => {
                    self.quads.push(solid(x, y, cw, t, c.color));
                    self.quads.push(solid(x, y + ch - t, cw, t, c.color));
                    self.quads.push(solid(x, y, t, ch, c.color));
                    self.quads.push(solid(x + cw - t, y, t, ch, c.color));
                }
                _ => {} // block is drawn as the cell's background
            }
        }

    }

    /// Queue one line of interface text, cut with `…` when wider than `max_w`.
    #[allow(clippy::too_many_arguments)]
    fn push_label(&mut self, fonts: &mut Fonts, x: f32, y: f32, text: &str, face: Face, px: f32, bold: bool, color: Rgb, max_w: f32) {
        let mut run = fonts.run(text, face, px, bold);
        if run.width > max_w {
            // Binary search the longest prefix that fits with an ellipsis.
            let chars: Vec<char> = text.chars().collect();
            let (mut lo, mut hi) = (0, chars.len());
            while lo < hi {
                let mid = (lo + hi).div_ceil(2);
                let t: String = chars[..mid].iter().collect::<String>().trim_end().to_string() + "…";
                if fonts.run(&t, face, px, bold).width <= max_w { lo = mid } else { hi = mid - 1 }
            }
            let t: String = chars[..lo].iter().collect::<String>().trim_end().to_string() + "…";
            run = fonts.run(&t, face, px, bold);
        }
        for (key, gx, gy) in run.glyphs {
            if let Some(e) = self.glyph(fonts, AtlasKey::Run(key)) {
                self.quads.push(Quad {
                    pos: [(x + gx as f32 + e.left as f32).round(), (y + gy as f32 - e.top as f32).round()],
                    size: [e.w as f32, e.h as f32],
                    uv: [e.x as f32, e.y as f32, e.w as f32, e.h as f32],
                    color: rgba(color),
                    kind: if e.color { COLOR } else { MASK },
                    _pad: [0; 3],
                });
            }
        }
    }

    /// Draw chrome (behind) and every pane frame. Returns true when another frame
    /// is needed (the atlas was reset mid-frame).
    pub fn render(&mut self, frames: &[(&Frame, (f32, f32))], chrome: &[Chrome], fonts: &mut Fonts, clear: Rgb) -> bool {
        self.atlas_reset = false;
        self.quads.clear();
        for item in chrome {
            match item {
                Chrome::Rect { rect, color } => self.quads.push(solid(rect.x, rect.y, rect.w, rect.h, *color)),
                Chrome::Round { rect, radius, color } => self.quads.push(Quad {
                    pos: [rect.x, rect.y],
                    size: [rect.w, rect.h],
                    uv: [radius.min(rect.w / 2.0).min(rect.h / 2.0), 0.0, 0.0, 0.0],
                    color: rgba(*color),
                    kind: ROUND,
                    _pad: [0; 3],
                }),
                Chrome::Label { x, y, text, face, px, bold, color, max_w } => {
                    self.push_label(fonts, *x, *y, text, *face, *px, *bold, *color, *max_w)
                }
            }
        }
        for (frame, pad) in frames {
            self.push_frame(frame, fonts, *pad);
        }
        let screen = [self.config.width as f32, self.config.height as f32, atlas::SIZE as f32, atlas::SIZE as f32];
        self.queue.write_buffer(&self.uniforms, 0, bytemuck::cast_slice(&screen));
        let needed = (self.quads.len() * std::mem::size_of::<Quad>()) as u64;
        if needed > self.instances.size() {
            self.instances = Self::instance_buffer(&self.device, self.quads.len().next_power_of_two());
        }
        self.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&self.quads));

        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => {
                self.surface.configure(&self.device, &self.config);
                return true;
            }
        };
        let view = output.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let [r, g, b] = clear.map(|v| v as f64 / 255.0);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a: 1.0 }), store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.instances.slice(..needed.max(1)));
            pass.draw(0..4, 0..self.quads.len() as u32);
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(output);
        self.atlas_reset
    }
}

#[cfg(test)]
mod preedit_tests {
    use super::*;

    #[test]
    fn preedit_replaces_cells_and_moves_cursor() {
        let cell = |col, c| CellDraw { col, line: 0, c, fg: theme::FOREGROUND, bg: theme::BACKGROUND, flags: Flags::empty() };
        let cursor = CursorDraw { col: 1, line: 0, shape: CursorShape::Block, color: theme::ACCENT };
        let mut f = Frame { cells: vec![cell(0, 'a'), cell(1, 'b'), cell(2, 'c')], cursor: Some(cursor) };
        f.overlay_preedit("´");
        let mut got: Vec<_> = f.cells.iter().map(|c| (c.col, c.c, c.flags.contains(Flags::UNDERLINE))).collect();
        got.sort();
        assert_eq!(got, [(0, 'a', false), (1, '´', true), (2, 'c', false)]);
        assert_eq!(f.cursor.unwrap().col, 2);
    }
}
