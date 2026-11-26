mod config;
mod font;
mod input;
mod renderer;
mod session;
mod theme;
mod welcome;

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
use winit::keyboard::{Key, ModifiersState};
use winit::platform::macos::WindowAttributesExtMacOS;
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::window::{Window, WindowId};

use config::Config;
use font::Fonts;
use renderer::{Frame, Renderer};
use session::{Listener, Session, UserEvent};

/// Logical padding: room for the traffic lights over the transparent titlebar.
const PAD_X: f32 = 12.0;
const PAD_TOP: f32 = 34.0;
const PAD_BOTTOM: f32 = 8.0;
const BLINK: Duration = Duration::from_millis(530);
const MULTI_CLICK: Duration = Duration::from_millis(400);

struct State {
    window: Arc<Window>,
    renderer: Renderer,
    fonts: Fonts,
    session: Session,
    clipboard: Option<arboard::Clipboard>,
    font_points: f32,
    mods: ModifiersState,
    focused: bool,
    cursor_on: bool,
    next_blink: Instant,
    mouse: PhysicalPosition<f64>,
    selecting: bool,
    last_click: Option<(Instant, Point, u8)>,
    scroll_px: f64,
    pressed_button: Option<u8>,
}

struct App {
    config: Config,
    proxy: EventLoopProxy<UserEvent>,
    state: Option<State>,
}

impl State {
    fn pad(&self) -> (f32, f32) {
        let s = self.window.scale_factor() as f32;
        (PAD_X * s, PAD_TOP * s)
    }

    fn window_size(&self) -> WindowSize {
        let size = self.window.inner_size();
        let s = self.window.scale_factor() as f32;
        let (cw, ch) = (self.fonts.cell_w, self.fonts.cell_h);
        let cols = ((size.width as f32 - 2.0 * PAD_X * s) / cw).floor().max(2.0);
        let lines = ((size.height as f32 - (PAD_TOP + PAD_BOTTOM) * s) / ch).floor().max(1.0);
        WindowSize { num_cols: cols as u16, num_lines: lines as u16, cell_width: cw as u16, cell_height: ch as u16 }
    }

    fn relayout(&mut self) {
        let size = self.window.inner_size();
        self.renderer.resize(size.width, size.height);
        self.session.resize(self.window_size());
        self.window.request_redraw();
    }

    fn set_font_size(&mut self, points: f32) {
        self.font_points = points.clamp(6.0, 72.0);
        self.fonts.set_size(self.font_points, self.window.scale_factor() as f32);
        self.renderer.clear_glyphs();
        self.relayout();
    }

    /// Grid point under the mouse (viewport line adjusted for scrollback).
    fn point_at_mouse(&self) -> (Point, Side, usize, usize) {
        let (px, py) = self.pad();
        let ws = self.window_size();
        let x = (self.mouse.x as f32 - px) / self.fonts.cell_w;
        let y = (self.mouse.y as f32 - py) / self.fonts.cell_h;
        let col = (x.max(0.0) as usize).min(ws.num_cols as usize - 1);
        let line = (y.max(0.0) as usize).min(ws.num_lines as usize - 1);
        let side = if x.fract() < 0.5 { Side::Left } else { Side::Right };
        let offset = self.session.term.lock().grid().display_offset() as i32;
        (Point::new(Line(line as i32 - offset), Column(col)), side, col, line)
    }

    fn reset_blink(&mut self) {
        self.cursor_on = true;
        self.next_blink = Instant::now() + BLINK;
    }

    fn mouse_report(&self, button: u8, pressed: bool, motion: bool) {
        let (_, _, col, line) = self.point_at_mouse();
        let mode = *self.session.term.lock().mode();
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
        self.session.write(bytes);
    }

    fn mouse_mode(&self) -> bool {
        !self.mods.shift_key() && self.session.term.lock().mode().intersects(TermMode::MOUSE_MODE)
    }

    fn copy(&mut self) {
        let text = self.session.term.lock().selection_to_string();
        if let (Some(text), Some(cb)) = (text, self.clipboard.as_mut()) {
            let _ = cb.set_text(text);
        }
    }

    fn paste(&mut self) {
        let Some(text) = self.clipboard.as_mut().and_then(|cb| cb.get_text().ok()) else { return };
        let bracketed = self.session.term.lock().mode().contains(TermMode::BRACKETED_PASTE);
        let bytes = if bracketed {
            // Strip any end marker inside the text so it cannot break out of the paste.
            format!("\x1b[200~{}\x1b[201~", text.replace("\x1b[201~", ""))
        } else {
            text.replace("\r\n", "\r").replace('\n', "\r")
        };
        self.session.write(bytes.into_bytes());
        self.session.term.lock().scroll_display(Scroll::Bottom);
    }

    fn scroll(&mut self, lines: i32) {
        if lines == 0 {
            return;
        }
        let mode = *self.session.term.lock().mode();
        if mode.intersects(TermMode::MOUSE_MODE) && !self.mods.shift_key() {
            let button = if lines > 0 { 64 } else { 65 };
            for _ in 0..lines.unsigned_abs().min(10) {
                self.mouse_report(button, true, false);
            }
        } else if mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL) {
            let key = if lines > 0 { "A" } else { "B" };
            let prefix = if mode.contains(TermMode::APP_CURSOR) { "\x1bO" } else { "\x1b[" };
            self.session.write(format!("{prefix}{key}").repeat(lines.unsigned_abs() as usize).into_bytes());
        } else {
            self.session.term.lock().scroll_display(Scroll::Delta(lines));
            self.window.request_redraw();
        }
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
        let scale = window.scale_factor() as f32;
        let fonts = Fonts::new(&self.config.font.family, self.config.font.size, scale);
        let renderer = Renderer::new(window.clone());

        let mut state_size = {
            let size = window.inner_size();
            let cols = ((size.width as f32 - 2.0 * PAD_X * scale) / fonts.cell_w).floor().max(2.0);
            let lines = ((size.height as f32 - (PAD_TOP + PAD_BOTTOM) * scale) / fonts.cell_h).floor().max(1.0);
            WindowSize { num_cols: cols as u16, num_lines: lines as u16, cell_width: fonts.cell_w as u16, cell_height: fonts.cell_h as u16 }
        };
        state_size.num_lines = state_size.num_lines.max(1);
        let banner = self.config.welcome.then(|| {
            let info = welcome::SysInfo::read(self.config.user_name.as_deref(), &fonts.family);
            welcome::banner(&info, state_size.num_cols as usize)
        });
        let session = match Session::spawn(state_size, self.config.scrollback, Listener(self.proxy.clone()), banner) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("cardumen: cannot start shell: {e}");
                event_loop.exit();
                return;
            }
        };
        self.state = Some(State {
            window,
            renderer,
            fonts,
            session,
            clipboard: arboard::Clipboard::new().ok(),
            font_points: self.config.font.size,
            mods: ModifiersState::empty(),
            focused: true,
            cursor_on: true,
            next_blink: Instant::now() + BLINK,
            mouse: PhysicalPosition::new(0.0, 0.0),
            selecting: false,
            last_click: None,
            scroll_px: 0.0,
            pressed_button: None,
        });
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        let Some(st) = self.state.as_mut() else { return };
        let UserEvent::Term(event) = event;
        match event {
            Event::Wakeup | Event::CursorBlinkingChange | Event::MouseCursorDirty => st.window.request_redraw(),
            Event::Title(t) => st.window.set_title(&t),
            Event::ResetTitle => st.window.set_title("cardumen"),
            Event::ClipboardStore(_, text) => {
                if let Some(cb) = st.clipboard.as_mut() {
                    let _ = cb.set_text(text);
                }
            }
            Event::ClipboardLoad(_, format) => {
                let text = st.clipboard.as_mut().and_then(|cb| cb.get_text().ok()).unwrap_or_default();
                st.session.write(format(&text).into_bytes());
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
                st.session.write(reply.into_bytes());
            }
            Event::PtyWrite(text) => st.session.write(text.into_bytes()),
            Event::TextAreaSizeRequest(format) => {
                let size = st.window_size();
                st.session.write(format(size).into_bytes());
            }
            Event::Bell => st.window.request_user_attention(None),
            Event::Exit | Event::ChildExit(_) => event_loop.exit(),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(st) = self.state.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) => st.relayout(),
            WindowEvent::ScaleFactorChanged { .. } => {
                let points = st.font_points;
                st.set_font_size(points);
            }
            WindowEvent::ModifiersChanged(m) => st.mods = m.state(),
            WindowEvent::Focused(focused) => {
                st.focused = focused;
                if st.session.term.lock().mode().contains(TermMode::FOCUS_IN_OUT) {
                    st.session.write(if focused { &b"\x1b[I"[..] } else { &b"\x1b[O"[..] });
                }
                st.reset_blink();
                st.window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if st.mods.super_key() {
                    if let Key::Character(c) = &event.logical_key {
                        match c.as_str() {
                            "c" => st.copy(),
                            "v" => st.paste(),
                            "q" => event_loop.exit(),
                            "=" | "+" => st.set_font_size(st.font_points + 1.0),
                            "-" => st.set_font_size(st.font_points - 1.0),
                            "0" => st.set_font_size(self.config.font.size),
                            _ => {}
                        }
                        return;
                    }
                }
                let app_cursor = st.session.term.lock().mode().contains(TermMode::APP_CURSOR);
                let plain = event.key_without_modifiers();
                if let Some(bytes) = input::encode(&event.logical_key, &plain, event.text.as_deref(), st.mods, app_cursor) {
                    st.session.write(bytes);
                    let mut term = st.session.term.lock();
                    term.scroll_display(Scroll::Bottom);
                    term.selection = None;
                    drop(term);
                    st.reset_blink();
                    st.window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                st.mouse = position;
                if st.selecting {
                    let (point, side, _, _) = st.point_at_mouse();
                    if let Some(sel) = st.session.term.lock().selection.as_mut() {
                        sel.update(point, side);
                    }
                    st.window.request_redraw();
                } else if let Some(b) = st.pressed_button {
                    if st.session.term.lock().mode().intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION) {
                        st.mouse_report(b, true, true);
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let code = match button {
                    MouseButton::Left => 0,
                    MouseButton::Middle => 1,
                    MouseButton::Right => 2,
                    _ => return,
                };
                let pressed = state == ElementState::Pressed;
                if st.mouse_mode() {
                    st.pressed_button = pressed.then_some(code);
                    st.mouse_report(code, pressed, false);
                    return;
                }
                if button != MouseButton::Left {
                    return;
                }
                if !pressed {
                    st.selecting = false;
                    return;
                }
                let (point, side, _, _) = st.point_at_mouse();
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
                st.session.term.lock().selection = Some(Selection::new(ty, point, side));
                st.selecting = true;
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
                st.scroll(lines);
            }
            WindowEvent::RedrawRequested => {
                let frame = {
                    let term = st.session.term.lock();
                    Frame::capture(&term, st.focused, st.cursor_on)
                };
                let pad = st.pad();
                if st.renderer.render(&frame, &mut st.fonts, pad) {
                    st.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(st) = self.state.as_mut() else { return };
        let blinking = st.focused && st.session.term.lock().cursor_style().blinking;
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
