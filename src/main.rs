mod config;
mod font;
mod input;
mod layout;
mod renderer;
mod session;
mod theme;
mod welcome;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use alacritty_terminal::event::{Event, WindowSize};
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};
use winit::platform::macos::WindowAttributesExtMacOS;
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::window::{Window, WindowId};

use config::Config;
use font::Fonts;
use layout::{Arrow, Dir, PaneId, Rect, Tabs};
use renderer::{Chrome, Frame, Renderer};
use session::{Listener, Session, UserEvent};
use theme::Rgb;

// Logical sizes (points); multiplied by the scale factor when used.
/// Titlebar band holding the tabs, next to the traffic lights.
const BAR_H: f32 = 34.0;
const TABS_X: f32 = 80.0;
const TAB_MAX_W: f32 = 180.0;
const TAB_PAD: f32 = 12.0;
const PLUS_W: f32 = 28.0;
/// Inner padding of each pane.
const PAD: f32 = 8.0;
const DIVIDER: f32 = 1.0;

const HAIRLINE: Rgb = [0x1F, 0x33, 0x3D];
const TAB_BG: Rgb = [0x0F, 0x1F, 0x27];

const BLINK: Duration = Duration::from_millis(530);
const MULTI_CLICK: Duration = Duration::from_millis(400);

struct Pane {
    session: Session,
    title: Option<String>,
    rect: Rect,
}

enum TabHit {
    Tab(usize),
    New,
}

struct State {
    window: Arc<Window>,
    renderer: Renderer,
    fonts: Fonts,
    config: Config,
    proxy: EventLoopProxy<UserEvent>,
    clipboard: Option<arboard::Clipboard>,
    font_points: f32,
    panes: HashMap<PaneId, Pane>,
    tabs: Tabs,
    next_id: PaneId,
    tab_hits: Vec<(Rect, TabHit)>,
    mods: ModifiersState,
    window_focused: bool,
    cursor_on: bool,
    next_blink: Instant,
    mouse: PhysicalPosition<f64>,
    selecting: Option<PaneId>,
    last_click: Option<(Instant, Point, u8)>,
    scroll_px: f64,
    pressed_button: Option<(PaneId, u8)>,
    /// Tab being renamed and its edit buffer.
    renaming: Option<(usize, String)>,
    last_tab_click: Option<(Instant, usize)>,
}

struct App {
    config: Config,
    proxy: EventLoopProxy<UserEvent>,
    state: Option<State>,
}

impl State {
    fn scale(&self) -> f32 {
        self.window.scale_factor() as f32
    }

    fn content_rect(&self) -> Rect {
        let size = self.window.inner_size();
        let top = BAR_H * self.scale();
        Rect { x: 0.0, y: top, w: size.width as f32, h: (size.height as f32 - top).max(1.0) }
    }

    fn grid_size(&self, r: Rect) -> WindowSize {
        let pad = 2.0 * PAD * self.scale();
        let (cw, ch) = (self.fonts.cell_w, self.fonts.cell_h);
        WindowSize {
            num_cols: ((r.w - pad) / cw).floor().max(2.0) as u16,
            num_lines: ((r.h - pad) / ch).floor().max(1.0) as u16,
            cell_width: cw as u16,
            cell_height: ch as u16,
        }
    }

    fn grid_origin(&self, r: Rect) -> (f32, f32) {
        let pad = PAD * self.scale();
        (r.x + pad, r.y + pad)
    }

    fn tab_layout(&self, tab: usize) -> (Vec<(PaneId, Rect)>, Vec<Rect>) {
        let (mut panes, mut dividers) = (vec![], vec![]);
        self.tabs.list[tab].root.layout(self.content_rect(), DIVIDER * self.scale(), &mut panes, &mut dividers);
        (panes, dividers)
    }

    fn focused_id(&self) -> PaneId {
        self.tabs.active().focused
    }

    fn focused(&self) -> &Pane {
        &self.panes[&self.focused_id()]
    }

    /// Give every pane its rect and resize PTYs whose rect changed.
    fn relayout(&mut self) {
        for t in 0..self.tabs.list.len() {
            for (id, rect) in self.tab_layout(t).0 {
                let size = self.grid_size(rect);
                if let Some(p) = self.panes.get_mut(&id) {
                    if p.rect != rect {
                        p.rect = rect;
                        p.session.resize(size);
                    }
                }
            }
        }
        self.update_title();
        self.window.request_redraw();
    }

    /// Start a shell for `id`, which must already be in the active tab's tree.
    fn spawn_pane(&mut self, id: PaneId, banner: bool, cwd: Option<PathBuf>) -> bool {
        let Some(rect) = self.tab_layout(self.tabs.active).0.into_iter().find(|(p, _)| *p == id).map(|(_, r)| r) else {
            return false;
        };
        let size = self.grid_size(rect);
        let banner = (banner && self.config.welcome).then(|| {
            let monitor = self.window.current_monitor();
            let display = monitor.map(|m| {
                let s = m.size().to_logical::<u32>(m.scale_factor());
                let hz = m.refresh_rate_millihertz().map_or(String::new(), |r| format!(", {} Hz", (r + 500) / 1000));
                // winit falls back to "Monitor #<id>" when macOS gives no name; that adds nothing.
                let name = m.name().filter(|n| !n.starts_with("Monitor #")).map_or(String::new(), |n| format!("{n} · "));
                format!("{name}{}×{} @ {}x{hz}", s.width, s.height, m.scale_factor())
            });
            let font = format!("{} ({}pt)", self.fonts.family, self.font_points);
            let info = welcome::SysInfo::read(self.config.user_name.as_deref(), &font, display);
            welcome::banner(&info, size.num_cols as usize)
        });
        match Session::spawn(size, self.config.scrollback, Listener(self.proxy.clone(), id), banner, cwd) {
            Ok(session) => {
                self.panes.insert(id, Pane { session, title: None, rect });
                true
            }
            Err(e) => {
                eprintln!("cardumen: cannot start shell: {e}");
                false
            }
        }
    }

    fn take_id(&mut self) -> PaneId {
        self.next_id += 1;
        self.next_id
    }

    fn focused_cwd(&self) -> Option<PathBuf> {
        self.panes.get(&self.tabs.list.get(self.tabs.active)?.focused)?.session.cwd()
    }

    fn new_tab(&mut self) {
        let cwd = if self.tabs.list.is_empty() { None } else { self.focused_cwd() };
        let id = self.take_id();
        self.tabs.open(id);
        if !self.spawn_pane(id, true, cwd) {
            self.tabs.remove_pane(id);
        }
        self.relayout();
    }

    fn split(&mut self, dir: Dir) {
        let cwd = self.focused_cwd();
        let (target, id) = (self.focused_id(), self.take_id());
        self.tabs.active_mut().root.split(target, id, dir);
        self.tabs.active_mut().focused = id;
        if !self.spawn_pane(id, false, cwd) {
            self.tabs.remove_pane(id);
        }
        self.relayout();
    }

    /// Returns true when no tabs are left (the app should quit).
    fn close_pane(&mut self, id: PaneId) -> bool {
        if self.panes.remove(&id).is_none() {
            return self.tabs.list.is_empty();
        }
        self.renaming = None;
        self.tabs.remove_pane(id);
        if self.tabs.list.is_empty() {
            return true;
        }
        self.reset_blink();
        self.relayout();
        false
    }

    fn tab_title(&self, i: usize) -> String {
        if let Some(name) = &self.tabs.list[i].name {
            return name.clone();
        }
        let Some(pane) = self.panes.get(&self.tabs.list[i].focused) else { return String::new() };
        // Shell prompts set "user@host: path" on every tab; the folder name says more.
        let user_prefix = format!("{}@", std::env::var("USER").unwrap_or_default());
        if let Some(t) = pane.title.as_deref().filter(|t| !t.is_empty() && !t.starts_with(&user_prefix)) {
            return t.to_string();
        }
        let home = std::env::var_os("HOME").map(PathBuf::from);
        match pane.session.cwd() {
            Some(p) if Some(&p) == home.as_ref() => "~".into(),
            Some(p) => p.file_name().map_or("/".into(), |n| n.to_string_lossy().into_owned()),
            None => "zsh".into(),
        }
    }

    fn update_title(&self) {
        if !self.tabs.list.is_empty() {
            self.window.set_title(&self.tab_title(self.tabs.active));
        }
    }

    /// Tab bar and dividers. Also records tab hit-boxes for clicks.
    fn chrome(&mut self) -> Vec<Chrome> {
        let s = self.scale();
        let width = self.window.inner_size().width as f32;
        let (cw, ch) = (self.fonts.cell_w, self.fonts.cell_h);
        let mut out = vec![Chrome::Rect {
            rect: Rect { x: 0.0, y: BAR_H * s - s, w: width, h: s },
            color: HAIRLINE,
        }];
        self.tab_hits.clear();
        let n = self.tabs.list.len() as f32;
        let room = width - TABS_X * s - PLUS_W * s - 2.0 * PAD * s;
        let tab_w = (room / n).min(TAB_MAX_W * s).floor();
        let (top, h) = (5.0 * s, BAR_H * s - 6.0 * s);
        let text_y = top + ((h - ch) / 2.0).round();
        for i in 0..self.tabs.list.len() {
            let x = TABS_X * s + i as f32 * tab_w;
            let rect = Rect { x, y: top, w: tab_w - 2.0 * s, h };
            let active = i == self.tabs.active;
            if active {
                out.push(Chrome::Rect { rect, color: TAB_BG });
                out.push(Chrome::Rect {
                    rect: Rect { x, y: top + h - 2.0 * s, w: rect.w, h: 2.0 * s },
                    color: theme::ACCENT,
                });
            }
            let max_w = rect.w - 2.0 * TAB_PAD * s;
            let (text, editing) = match &self.renaming {
                Some((j, buf)) if *j == i => (buf.clone(), true),
                _ => (self.tab_title(i), false),
            };
            if editing {
                // Caret after the text (clamped to the tab), in the accent colour.
                let caret_x = x + TAB_PAD * s + (text.chars().count() as f32 * cw).min(max_w);
                out.push(Chrome::Rect { rect: Rect { x: caret_x, y: text_y, w: 2.0 * s, h: ch }, color: theme::ACCENT });
            }
            out.push(Chrome::Text {
                x: x + TAB_PAD * s,
                y: text_y,
                text,
                color: if active { theme::FOREGROUND } else { theme::MUTED },
                max_w,
            });
            self.tab_hits.push((rect, TabHit::Tab(i)));
        }
        let plus_x = TABS_X * s + n * tab_w;
        out.push(Chrome::Text {
            x: plus_x + ((PLUS_W * s - cw) / 2.0).round(),
            y: text_y,
            text: "+".into(),
            color: theme::MUTED,
            max_w: cw,
        });
        self.tab_hits.push((Rect { x: plus_x, y: top, w: PLUS_W * s, h }, TabHit::New));
        let (_, dividers) = self.tab_layout(self.tabs.active);
        out.extend(dividers.into_iter().map(|rect| Chrome::Rect { rect, color: HAIRLINE }));
        out
    }

    fn pane_at(&self, x: f32, y: f32) -> Option<PaneId> {
        self.tab_layout(self.tabs.active).0.into_iter().find(|(_, r)| r.contains(x, y)).map(|(id, _)| id)
    }

    /// Grid point under the mouse inside `id` (line adjusted for scrollback).
    fn point_in(&self, id: PaneId) -> (Point, Side, usize, usize) {
        let pane = &self.panes[&id];
        let (ox, oy) = self.grid_origin(pane.rect);
        let ws = self.grid_size(pane.rect);
        let x = (self.mouse.x as f32 - ox) / self.fonts.cell_w;
        let y = (self.mouse.y as f32 - oy) / self.fonts.cell_h;
        let col = (x.max(0.0) as usize).min(ws.num_cols as usize - 1);
        let line = (y.max(0.0) as usize).min(ws.num_lines as usize - 1);
        let side = if x.fract() < 0.5 { Side::Left } else { Side::Right };
        let offset = pane.session.term.lock().grid().display_offset() as i32;
        (Point::new(Line(line as i32 - offset), Column(col)), side, col, line)
    }

    fn reset_blink(&mut self) {
        self.cursor_on = true;
        self.next_blink = Instant::now() + BLINK;
    }

    fn mouse_report(&self, id: PaneId, button: u8, pressed: bool, motion: bool) {
        let (_, _, col, line) = self.point_in(id);
        let session = &self.panes[&id].session;
        let mode = *session.term.lock().mode();
        let mut code = button + if motion { 32 } else { 0 };
        if self.mods.shift_key() {
            code += 4;
        }
        if self.mods.alt_key() {
            code += 8;
        }
        if self.mods.control_key() {
            code += 16;
        }
        let bytes = if mode.contains(TermMode::SGR_MOUSE) {
            format!("\x1b[<{code};{};{}{}", col + 1, line + 1, if pressed { 'M' } else { 'm' }).into_bytes()
        } else {
            let code = if pressed { code } else { 3 + (code & !3) };
            vec![0x1b, b'[', b'M', 32 + code, 32 + (col as u8).saturating_add(1), 32 + (line as u8).saturating_add(1)]
        };
        session.write(bytes);
    }

    fn mouse_mode(&self, id: PaneId) -> bool {
        !self.mods.shift_key() && self.panes[&id].session.term.lock().mode().intersects(TermMode::MOUSE_MODE)
    }

    fn copy(&mut self) {
        let text = self.focused().session.term.lock().selection_to_string();
        if let (Some(text), Some(cb)) = (text, self.clipboard.as_mut()) {
            let _ = cb.set_text(text);
        }
    }

    fn paste(&mut self) {
        let Some(text) = self.clipboard.as_mut().and_then(|cb| cb.get_text().ok()) else { return };
        let session = &self.panes[&self.focused_id()].session;
        let bracketed = session.term.lock().mode().contains(TermMode::BRACKETED_PASTE);
        let bytes = if bracketed {
            // Strip any end marker inside the text so it cannot break out of the paste.
            format!("\x1b[200~{}\x1b[201~", text.replace("\x1b[201~", ""))
        } else {
            text.replace("\r\n", "\r").replace('\n', "\r")
        };
        session.write(bytes.into_bytes());
        session.term.lock().scroll_display(Scroll::Bottom);
    }

    fn scroll(&mut self, id: PaneId, lines: i32) {
        if lines == 0 {
            return;
        }
        let session = &self.panes[&id].session;
        let mode = *session.term.lock().mode();
        if mode.intersects(TermMode::MOUSE_MODE) && !self.mods.shift_key() {
            let button = if lines > 0 { 64 } else { 65 };
            for _ in 0..lines.unsigned_abs().min(10) {
                self.mouse_report(id, button, true, false);
            }
        } else if mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL) {
            let key = if lines > 0 { "A" } else { "B" };
            let prefix = if mode.contains(TermMode::APP_CURSOR) { "\x1bO" } else { "\x1b[" };
            session.write(format!("{prefix}{key}").repeat(lines.unsigned_abs() as usize).into_bytes());
        } else {
            session.term.lock().scroll_display(Scroll::Delta(lines));
            self.window.request_redraw();
        }
    }

    fn set_font_size(&mut self, points: f32) {
        self.font_points = points.clamp(6.0, 72.0);
        self.fonts.set_size(self.font_points, self.scale());
        self.renderer.clear_glyphs();
        for p in self.panes.values_mut() {
            p.rect = Rect::default(); // force a PTY resize: the cell size changed
        }
        self.relayout();
    }

    fn focus(&mut self, id: PaneId) {
        if self.focused_id() != id {
            self.tabs.active_mut().focused = id;
            self.reset_blink();
            self.update_title();
            self.window.request_redraw();
        }
    }

    /// Cmd shortcuts. Returns true when the key was consumed.
    fn shortcut(&mut self, key: &Key, plain: &Key, physical: PhysicalKey, event_loop: &ActiveEventLoop) -> bool {
        let m = self.mods;
        if m.control_key() && !m.super_key() && *key == Key::Named(NamedKey::Tab) {
            self.tabs.cycle(!m.shift_key());
            self.after_tab_change();
            return true;
        }
        if !m.super_key() {
            return false;
        }
        if m.alt_key() {
            let arrow = match key {
                Key::Named(NamedKey::ArrowLeft) => Arrow::Left,
                Key::Named(NamedKey::ArrowRight) => Arrow::Right,
                Key::Named(NamedKey::ArrowUp) => Arrow::Up,
                Key::Named(NamedKey::ArrowDown) => Arrow::Down,
                _ => return false,
            };
            let (rects, _) = self.tab_layout(self.tabs.active);
            if let Some(id) = layout::neighbour(&rects, self.focused_id(), arrow) {
                self.focus(id);
            }
            return true;
        }
        match physical {
            PhysicalKey::Code(KeyCode::BracketLeft) if m.shift_key() => {
                self.tabs.cycle(false);
                self.after_tab_change();
                return true;
            }
            PhysicalKey::Code(KeyCode::BracketRight) if m.shift_key() => {
                self.tabs.cycle(true);
                self.after_tab_change();
                return true;
            }
            _ => {}
        }
        let Key::Character(c) = plain else { return false };
        match c.to_lowercase().as_str() {
            "c" => self.copy(),
            "v" => self.paste(),
            "q" => event_loop.exit(),
            "t" => self.new_tab(),
            "w" => {
                if self.close_pane(self.focused_id()) {
                    event_loop.exit();
                }
            }
            "d" => self.split(if m.shift_key() { Dir::Down } else { Dir::Right }),
            "r" if m.shift_key() => self.start_rename(self.tabs.active),
            "=" | "+" => self.set_font_size(self.font_points + 1.0),
            "-" => self.set_font_size(self.font_points - 1.0),
            "0" => self.set_font_size(self.config.font.size),
            d @ ("1" | "2" | "3" | "4" | "5" | "6" | "7" | "8") => {
                self.tabs.select(d.parse::<usize>().unwrap() - 1);
                self.after_tab_change();
            }
            "9" => {
                self.tabs.select(self.tabs.list.len() - 1);
                self.after_tab_change();
            }
            _ => return false,
        }
        true
    }

    fn start_rename(&mut self, i: usize) {
        self.commit_rename();
        self.tabs.select(i);
        let current = self.tabs.list[i].name.clone().unwrap_or_else(|| self.tab_title(i));
        self.renaming = Some((i, current));
        self.window.request_redraw();
    }

    /// Save the edited name; an empty name goes back to the automatic title.
    fn commit_rename(&mut self) {
        if let Some((i, name)) = self.renaming.take() {
            if let Some(tab) = self.tabs.list.get_mut(i) {
                let name = name.trim();
                tab.name = (!name.is_empty()).then(|| name.to_string());
            }
            self.update_title();
            self.window.request_redraw();
        }
    }

    fn after_tab_change(&mut self) {
        self.reset_blink();
        self.update_title();
        self.window.request_redraw();
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("cardumen")
            .with_inner_size(LogicalSize::new(980.0, 620.0))
            .with_titlebar_transparent(true)
            .with_fullsize_content_view(true)
            .with_title_hidden(true);
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let fonts = Fonts::new(&self.config.font.family, self.config.font.size, window.scale_factor() as f32);
        let renderer = Renderer::new(window.clone());
        let mut st = State {
            window,
            renderer,
            fonts,
            config: self.config.clone(),
            proxy: self.proxy.clone(),
            clipboard: arboard::Clipboard::new().ok(),
            font_points: self.config.font.size,
            panes: HashMap::new(),
            tabs: Tabs::default(),
            next_id: 0,
            tab_hits: vec![],
            mods: ModifiersState::empty(),
            window_focused: true,
            cursor_on: true,
            next_blink: Instant::now() + BLINK,
            mouse: PhysicalPosition::new(0.0, 0.0),
            selecting: None,
            last_click: None,
            scroll_px: 0.0,
            pressed_button: None,
            renaming: None,
            last_tab_click: None,
        };
        st.new_tab();
        if st.tabs.list.is_empty() {
            event_loop.exit();
            return;
        }
        self.state = Some(st);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        let Some(st) = self.state.as_mut() else { return };
        let UserEvent::Term(id, event) = event;
        if let Event::Exit | Event::ChildExit(_) = event {
            if st.close_pane(id) {
                event_loop.exit();
            }
            return;
        }
        let Some(pane) = st.panes.get_mut(&id) else { return };
        match event {
            Event::Wakeup | Event::CursorBlinkingChange | Event::MouseCursorDirty => st.window.request_redraw(),
            Event::Title(t) => {
                pane.title = Some(t);
                st.update_title();
                st.window.request_redraw();
            }
            Event::ResetTitle => {
                pane.title = None;
                st.update_title();
                st.window.request_redraw();
            }
            Event::ClipboardStore(_, text) => {
                if let Some(cb) = st.clipboard.as_mut() {
                    let _ = cb.set_text(text);
                }
            }
            Event::ClipboardLoad(_, format) => {
                let text = st.clipboard.as_mut().and_then(|cb| cb.get_text().ok()).unwrap_or_default();
                st.panes[&id].session.write(format(&text).into_bytes());
            }
            Event::ColorRequest(index, format) => {
                let rgb = match index {
                    256 => theme::FOREGROUND,
                    257 => theme::BACKGROUND,
                    258 => theme::ACCENT,
                    i if i < 256 => theme::indexed(i as u8),
                    _ => theme::FOREGROUND,
                };
                let reply = format(alacritty_terminal::vte::ansi::Rgb { r: rgb[0], g: rgb[1], b: rgb[2] });
                pane.session.write(reply.into_bytes());
            }
            Event::PtyWrite(text) => pane.session.write(text.into_bytes()),
            Event::TextAreaSizeRequest(format) => {
                let size = st.grid_size(st.panes[&id].rect);
                st.panes[&id].session.write(format(size).into_bytes());
            }
            Event::Bell => st.window.request_user_attention(None),
            Event::Exit | Event::ChildExit(_) => unreachable!("handled above"),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(st) = self.state.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                st.renderer.resize(size.width, size.height);
                st.relayout();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let points = st.font_points;
                st.set_font_size(points);
            }
            WindowEvent::ModifiersChanged(m) => st.mods = m.state(),
            WindowEvent::Focused(focused) => {
                st.window_focused = focused;
                let session = &st.focused().session;
                if session.term.lock().mode().contains(TermMode::FOCUS_IN_OUT) {
                    session.write(if focused { &b"\x1b[I"[..] } else { &b"\x1b[O"[..] });
                }
                st.reset_blink();
                st.window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if let Some((_, buf)) = st.renaming.as_mut() {
                    match input::edit_line(buf, &event.logical_key, event.text.as_deref(), st.mods) {
                        input::Edit::Continue => st.window.request_redraw(),
                        input::Edit::Commit => st.commit_rename(),
                        input::Edit::Cancel => {
                            st.renaming = None;
                            st.window.request_redraw();
                        }
                    }
                    return;
                }
                let plain = event.key_without_modifiers();
                if st.shortcut(&event.logical_key, &plain, event.physical_key, event_loop) {
                    return;
                }
                let session = &st.panes[&st.focused_id()].session;
                let app_cursor = session.term.lock().mode().contains(TermMode::APP_CURSOR);
                if let Some(bytes) = input::encode(&event.logical_key, &plain, event.text.as_deref(), st.mods, app_cursor) {
                    session.write(bytes);
                    let mut term = session.term.lock();
                    term.scroll_display(Scroll::Bottom);
                    term.selection = None;
                    drop(term);
                    st.reset_blink();
                    st.window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                st.mouse = position;
                if let Some(id) = st.selecting {
                    let (point, side, _, _) = st.point_in(id);
                    if let Some(sel) = st.panes[&id].session.term.lock().selection.as_mut() {
                        sel.update(point, side);
                    }
                    st.window.request_redraw();
                } else if let Some((id, b)) = st.pressed_button {
                    if st.panes.get(&id).is_some_and(|p| {
                        p.session.term.lock().mode().intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION)
                    }) {
                        st.mouse_report(id, b, true, true);
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;
                let (x, y) = (st.mouse.x as f32, st.mouse.y as f32);
                if pressed && button == MouseButton::Left && y < BAR_H * st.scale() {
                    match st.tab_hits.iter().find(|(r, _)| r.contains(x, y)).map(|(_, h)| h) {
                        Some(TabHit::Tab(i)) => {
                            let i = *i;
                            let now = Instant::now();
                            let double = st.last_tab_click.is_some_and(|(t, j)| j == i && now - t < MULTI_CLICK);
                            st.last_tab_click = Some((now, i));
                            if double {
                                st.start_rename(i);
                            } else if st.renaming.as_ref().is_none_or(|(j, _)| *j != i) {
                                st.commit_rename();
                                st.tabs.select(i);
                                st.after_tab_change();
                            }
                        }
                        Some(TabHit::New) => st.new_tab(),
                        None => {
                            let _ = st.window.drag_window();
                        }
                    }
                    return;
                }
                if pressed {
                    st.commit_rename();
                }
                if !pressed {
                    // Copy on select: whatever was just selected goes to the clipboard.
                    if let Some(id) = st.selecting.take() {
                        let text = st.panes.get(&id).and_then(|p| p.session.term.lock().selection_to_string());
                        if let (Some(text), Some(cb)) = (text.filter(|t| !t.is_empty()), st.clipboard.as_mut()) {
                            let _ = cb.set_text(text);
                        }
                    }
                    if let Some((id, code)) = st.pressed_button.take() {
                        if st.panes.contains_key(&id) {
                            st.mouse_report(id, code, false, false);
                        }
                    }
                    return;
                }
                let Some(id) = st.pane_at(x, y) else { return };
                st.focus(id);
                let code = match button {
                    MouseButton::Left => 0,
                    MouseButton::Middle => 1,
                    MouseButton::Right => 2,
                    _ => return,
                };
                if st.mouse_mode(id) {
                    st.pressed_button = Some((id, code));
                    st.mouse_report(id, code, true, false);
                    return;
                }
                if button != MouseButton::Left {
                    return;
                }
                let (point, side, _, _) = st.point_in(id);
                let now = Instant::now();
                let clicks = match st.last_click {
                    Some((t, p, n)) if now - t < MULTI_CLICK && p == point => n % 3 + 1,
                    _ => 1,
                };
                st.last_click = Some((now, point, clicks));
                let ty = match clicks {
                    2 => SelectionType::Semantic,
                    3 => SelectionType::Lines,
                    _ => SelectionType::Simple,
                };
                st.panes[&id].session.term.lock().selection = Some(Selection::new(ty, point, side));
                st.selecting = Some(id);
                st.window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => (y * 3.0) as i32,
                    MouseScrollDelta::PixelDelta(p) => {
                        st.scroll_px += p.y;
                        let ch = st.fonts.cell_h as f64;
                        let n = (st.scroll_px / ch).trunc();
                        st.scroll_px -= n * ch;
                        n as i32
                    }
                };
                let id = st.pane_at(st.mouse.x as f32, st.mouse.y as f32).unwrap_or(st.focused_id());
                st.scroll(id, lines);
            }
            WindowEvent::RedrawRequested => {
                let chrome = st.chrome();
                let focused = st.focused_id();
                let (rects, _) = st.tab_layout(st.tabs.active);
                let frames: Vec<(Frame, (f32, f32))> = rects
                    .iter()
                    .filter_map(|(id, rect)| {
                        let pane = st.panes.get(id)?;
                        let is_focused = *id == focused;
                        let term = pane.session.term.lock();
                        let frame = Frame::capture(&term, st.window_focused && is_focused, !is_focused || st.cursor_on);
                        Some((frame, st.grid_origin(*rect)))
                    })
                    .collect();
                let refs: Vec<(&Frame, (f32, f32))> = frames.iter().map(|(f, o)| (f, *o)).collect();
                if st.renderer.render(&refs, &chrome, &mut st.fonts) {
                    st.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(st) = self.state.as_mut() else { return };
        let blinking = st.window_focused && st.focused().session.term.lock().cursor_style().blinking;
        if !blinking {
            st.cursor_on = true;
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        if Instant::now() >= st.next_blink {
            st.cursor_on = !st.cursor_on;
            st.next_blink = Instant::now() + BLINK;
            st.window.request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(st.next_blink));
    }
}

fn main() {
    let config = config::load();
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    let proxy = event_loop.create_proxy();
    let mut app = App { config, proxy, state: None };
    event_loop.run_app(&mut app).expect("run");
}
