use crate::{
    audio::Audio,
    composition::{self, Action, Dialog, Interface},
    configuration::{Args, Preferences, Quality},
    memory::{Journal, Memory},
    platform::{data_directory, read_json, stamp, write_json},
    renderer::{self, Layer, Renderer, CANVAS},
    simulation::{World, DEFAULT_SEED, STEP},
    typography::Typography,
};
use anyhow::{Context, Result};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalSize},
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
    window::{Fullscreen, Window, WindowId},
};

pub fn run(args: Args) -> Result<()> {
    let directory = data_directory(args.data_dir.as_deref())?;
    let config_path = directory.join("preferences.json");
    let (mut preferences, config_available): (Preferences, bool) = match read_json(&config_path) {
        Ok(config) => (config, true),
        Err(error) => {
            eprintln!("antiOS / {error:#}");
            (Preferences::default(), false)
        }
    };
    preferences.normalize();
    if let Some(quality) = args.quality {
        preferences.quality = quality;
    }
    preferences.reduced_motion |= args.reduced_motion;
    let memory = Memory::load(&directory);
    let seed = args.seed.unwrap_or_else(|| {
        if memory.as_ref().is_ok_and(|history| history.seconds > 0.) {
            stamp()
        } else {
            DEFAULT_SEED
        }
    });
    let mut world = World::new(seed);
    match memory {
        Ok(history) => history.recall(&mut world),
        Err(error) => eprintln!("antiOS / {error:#}"),
    }
    let journal = Journal::new(directory.clone(), stamp(), &world);
    let event_loop = EventLoop::new().context("opening the window system")?;
    let mut app = App {
        args,
        directory,
        config_path,
        config_available,
        preferences,
        world,
        journal,
        renderer: None,
        window: None,
        typography: Typography::new(),
        art: Layer::default(),
        chrome: Layer::default(),
        audio: Audio::default(),
        dialog: Dialog::None,
        paused: false,
        pointer: None,
        modifiers: ModifiersState::empty(),
        status: None,
        capture: None,
        fatal: None,
        start: Instant::now(),
        last: Instant::now(),
        next: Instant::now(),
        time_bank: 0.,
        auto_quality: Quality::High,
        frame_cost: 0.016,
        adaptive_at: Instant::now(),
        saved_at: Instant::now(),
    };
    event_loop
        .run_app(&mut app)
        .context("running the installation")?;
    if let Some(error) = app.fatal {
        return Err(error);
    }
    Ok(())
}

struct App {
    args: Args,
    directory: PathBuf,
    config_path: PathBuf,
    config_available: bool,
    preferences: Preferences,
    world: World,
    journal: Journal,
    renderer: Option<Renderer>,
    window: Option<Arc<Window>>,
    typography: Typography,
    art: Layer,
    chrome: Layer,
    audio: Audio,
    dialog: Dialog,
    paused: bool,
    pointer: Option<glam::Vec2>,
    modifiers: ModifiersState,
    status: Option<(String, Instant)>,
    capture: Option<bool>,
    fatal: Option<anyhow::Error>,
    start: Instant,
    last: Instant,
    next: Instant,
    time_bank: f32,
    auto_quality: Quality,
    frame_cost: f32,
    adaptive_at: Instant,
    saved_at: Instant,
}

impl App {
    fn initialize(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let size = self.args.size.unwrap_or((1440, 900));
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("antiOS")
                    .with_inner_size(LogicalSize::new(size.0, size.1))
                    .with_min_inner_size(LogicalSize::new(620, 390))
                    .with_visible(false)
                    .with_fullscreen(self.args.exhibition.then_some(Fullscreen::Borderless(None)))
                    .with_window_icon(crate::platform::icon()),
            )?,
        );
        let dimensions = window.inner_size();
        let renderer = pollster::block_on(Renderer::new(
            Some(Arc::clone(&window)),
            (dimensions.width.max(1), dimensions.height.max(1)),
            self.args.backend,
            self.preferences.quality,
            &self.typography,
        ))?;
        self.auto_quality = match (renderer.adapter.device_type, renderer.adapter.backend) {
            (wgpu::DeviceType::Cpu, _) | (_, wgpu::Backend::Gl) => Quality::Low,
            (wgpu::DeviceType::IntegratedGpu, _) => Quality::Medium,
            _ => Quality::High,
        };
        eprintln!(
            "antiOS / {} ({:?})",
            renderer.adapter.name, renderer.adapter.backend
        );
        window.set_visible(true);
        self.start = Instant::now();
        self.last = self.start;
        self.next = self.start;
        self.adaptive_at = self.start;
        self.renderer = Some(renderer);
        self.window = Some(window);
        self.cursor_visibility();
        Ok(())
    }

    fn report(&mut self, message: impl Into<String>) {
        self.status = Some((message.into(), Instant::now() + Duration::from_secs(5)));
    }

    fn save_preferences(&mut self) {
        if self.config_available {
            if let Err(error) = write_json(&self.config_path, &self.preferences) {
                eprintln!("antiOS / {error:#}");
                self.report("PREFERENCES COULD NOT BE SAVED");
            }
        } else {
            self.report("PREFERENCES FILE COULD NOT BE READ");
        }
    }

    fn checkpoint(&mut self) -> Result<()> {
        self.saved_at = Instant::now();
        self.journal.checkpoint(&self.world)
    }

    fn finish(&mut self, loop_handle: &ActiveEventLoop) {
        if let Err(error) = self.checkpoint() {
            self.fatal.get_or_insert(error);
        }
        loop_handle.exit();
    }

    fn cursor_visibility(&self) {
        if let Some(window) = &self.window {
            let hidden = self.args.exhibition
                && !self.args.show_cursor
                && self.dialog == Dialog::None
                && window.fullscreen().is_some();
            window.set_cursor_visible(!hidden);
        }
    }

    fn fullscreen(&self) {
        if let Some(window) = &self.window {
            window.set_fullscreen(if window.fullscreen().is_some() {
                None
            } else {
                Some(Fullscreen::Borderless(None))
            });
        }
    }

    fn quality(&self) -> Quality {
        if self.preferences.quality == Quality::Auto {
            self.auto_quality
        } else {
            self.preferences.quality
        }
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        let quality = self.quality();
        if let Some(renderer) = &mut self.renderer {
            renderer.resize((size.width, size.height), quality);
        }
    }

    fn escape(&mut self) {
        if matches!(self.dialog, Dialog::Exit | Dialog::Reset) {
            self.dialog = Dialog::Controls;
        } else if self
            .window
            .as_ref()
            .is_some_and(|w| w.fullscreen().is_some())
        {
            self.fullscreen();
            self.dialog = Dialog::Controls;
        } else {
            self.dialog = if self.dialog == Dialog::None {
                Dialog::Controls
            } else {
                Dialog::None
            };
        }
    }

    fn action(&mut self, action: Action, loop_handle: &ActiveEventLoop) {
        if matches!(self.dialog, Dialog::Exit | Dialog::Reset)
            && !matches!(action, Action::Confirm | Action::Back)
        {
            return;
        }
        match action {
            Action::Pause => {
                self.paused = !self.paused;
                self.time_bank = 0.;
            }
            Action::Mute => {
                if let Err(error) = self.audio.toggle() {
                    eprintln!("antiOS / {error:#}");
                    self.report("SOUND OUTPUT UNAVAILABLE");
                }
            }
            Action::Fullscreen => self.fullscreen(),
            Action::Capture => self.capture = Some(self.modifiers.shift_key()),
            Action::Motion => {
                self.preferences.reduced_motion = !self.preferences.reduced_motion;
                self.save_preferences();
            }
            Action::Quality => {
                self.preferences.quality = self.preferences.quality.next();
                if let Some(window) = &self.window {
                    self.resize(window.inner_size());
                }
                self.save_preferences();
            }
            Action::Volume(value) => {
                self.preferences.volume = value.clamp(0., 1.);
                self.report(format!(
                    "VOLUME / {:02}%",
                    (self.preferences.volume * 100.).round() as u32
                ));
                self.save_preferences();
            }
            Action::Reset => self.dialog = Dialog::Reset,
            Action::Exit => self.dialog = Dialog::Exit,
            Action::Back => self.escape(),
            Action::Confirm if self.dialog == Dialog::Exit => self.finish(loop_handle),
            Action::Confirm if self.dialog == Dialog::Reset => {
                if let Err(error) = self.checkpoint() {
                    eprintln!("antiOS / {error:#}");
                    self.dialog = Dialog::Controls;
                    self.report("WORLD COULD NOT BE SAVED");
                    return;
                }
                let mut fresh = World::new(stamp());
                if let Ok(memory) = Memory::load(&self.directory) {
                    memory.recall(&mut fresh);
                }
                self.journal = Journal::new(self.directory.clone(), stamp(), &fresh);
                self.world = fresh;
                self.dialog = Dialog::None;
                self.paused = false;
                if let Some(r) = &mut self.renderer {
                    r.clear_history();
                }
            }
            Action::Confirm => {}
        }
        if self.dialog != Dialog::None {
            self.world.cursor.leave();
        }
        self.audio
            .follow(&self.world, self.preferences.volume, self.paused);
        self.cursor_visibility();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn frame(&mut self) -> Result<()> {
        let start = Instant::now();
        let elapsed = start.duration_since(self.last).as_secs_f32().min(0.25);
        self.last = start;
        if !self.paused {
            self.time_bank += elapsed;
            while self.time_bank >= STEP {
                self.world.step();
                self.time_bank -= STEP;
            }
        }
        if self.saved_at.elapsed() >= Duration::from_secs(18) {
            if let Err(error) = self.checkpoint() {
                eprintln!("antiOS / {error:#}");
            }
        }
        if self
            .status
            .as_ref()
            .is_some_and(|(_, until)| start >= *until)
        {
            self.status = None;
        }
        self.audio
            .follow(&self.world, self.preferences.volume, self.paused);
        let window = self.window.as_ref().context("window unavailable")?;
        let size = window.inner_size();
        let active_quality = self.quality();
        let renderer = self.renderer.as_mut().context("renderer unavailable")?;
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        if renderer.quality != active_quality {
            renderer.resize((size.width, size.height), active_quality);
        }

        let reduced = self.preferences.reduced_motion;
        composition::artwork(
            &mut self.art,
            &self.typography,
            &self.world,
            active_quality,
            reduced,
        );
        let ui = Interface {
            dialog: self.dialog,
            paused: self.paused,
            muted: self.audio.muted(),
            fullscreen: window.fullscreen().is_some(),
            preferences: &self.preferences,
            status: self.status.as_ref().map(|(message, _)| message.as_str()),
        };
        let frame = composition::frame(&self.world, reduced);
        if self.args.exhibition && self.dialog == Dialog::None && window.fullscreen().is_some() {
            self.chrome.clear();
        } else {
            composition::chrome(&mut self.chrome, &self.typography, &self.world, &ui);
        }
        renderer.draw(&self.art, &self.chrome, frame, true)?;

        if let Some(print) = self.capture.take() {
            let mut capture_chrome = Layer::default();
            if !self.args.clean {
                let still = Interface {
                    dialog: Dialog::None,
                    status: None,
                    ..ui
                };
                composition::chrome(&mut capture_chrome, &self.typography, &self.world, &still);
            }
            let dimensions = if print {
                (3840, 2400)
            } else {
                (
                    size.width.min(renderer.limit),
                    size.height.min(renderer.limit),
                )
            };
            let filename = format!("antiOS-{:08x}-{}.png", self.world.seed as u32, stamp());
            let path = self.directory.join("captures").join(filename);
            let result = if print {
                let mut print_art = Layer::default();
                composition::artwork(
                    &mut print_art,
                    &self.typography,
                    &self.world,
                    Quality::High,
                    reduced,
                );
                renderer.print(&path, frame, &print_art, &capture_chrome, dimensions)
            } else if self.dialog == Dialog::None && !self.args.clean && !self.args.exhibition {
                renderer.save(&path)
            } else {
                renderer.print(&path, frame, &self.art, &capture_chrome, dimensions)
            };
            match result {
                Ok(()) => {
                    eprintln!("antiOS / {}", path.display());
                    self.report("IMAGE SAVED / CAPTURES");
                }
                Err(error) => {
                    eprintln!("antiOS / {error:#}");
                    self.report("IMAGE COULD NOT BE SAVED");
                }
            }
        }
        let cost = start.elapsed().as_secs_f32();
        self.frame_cost += (cost - self.frame_cost) * 0.045;
        if self.preferences.quality == Quality::Auto
            && self.auto_quality != Quality::Low
            && self.adaptive_at.elapsed() > Duration::from_secs(4)
            && self.frame_cost > 0.026
        {
            self.auto_quality = match self.auto_quality {
                Quality::High => Quality::Medium,
                _ => Quality::Low,
            };
            self.adaptive_at = Instant::now();
        }
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            if let Err(error) = self.initialize(event_loop) {
                self.fatal = Some(error);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(window) = &self.window else {
            return;
        };
        if window.id() != id {
            return;
        }
        match event {
            WindowEvent::CloseRequested => self.finish(event_loop),
            WindowEvent::Resized(size) => {
                self.resize(size);
                self.cursor_visibility();
            }
            WindowEvent::Focused(false) => {
                self.world.cursor.leave();
                self.modifiers = ModifiersState::empty();
            }
            WindowEvent::CursorLeft { .. } => {
                self.pointer = None;
                self.world.cursor.leave();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let size = window.inner_size();
                self.pointer = renderer::pointer(position.x, position.y, (size.width, size.height));
                if let Some(p) = self
                    .pointer
                    .filter(|_| !self.args.exhibition && self.dialog == Dialog::None)
                {
                    self.world.cursor.move_to(p);
                } else {
                    self.world.cursor.leave();
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                let down = state == ElementState::Pressed;
                let visible = !self.args.exhibition
                    || self.dialog != Dialog::None
                    || window.fullscreen().is_none();
                if let Some(action) = self
                    .pointer
                    .filter(|_| down && visible)
                    .and_then(|p| composition::hit(p * CANVAS, self.dialog))
                {
                    self.action(action, event_loop);
                } else if self.dialog == Dialog::None && !self.args.exhibition {
                    self.world.cursor.press(down);
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                let action = match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => Action::Back,
                    PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => Action::Confirm,
                    PhysicalKey::Code(KeyCode::F11) => Action::Fullscreen,
                    PhysicalKey::Code(KeyCode::KeyM) => Action::Mute,
                    PhysicalKey::Code(KeyCode::Space) => Action::Pause,
                    PhysicalKey::Code(KeyCode::KeyS) => Action::Capture,
                    PhysicalKey::Code(KeyCode::KeyR) => Action::Reset,
                    PhysicalKey::Code(KeyCode::KeyQ) => Action::Exit,
                    PhysicalKey::Code(KeyCode::KeyD) => Action::Motion,
                    PhysicalKey::Code(KeyCode::KeyG) => Action::Quality,
                    PhysicalKey::Code(KeyCode::Minus | KeyCode::NumpadSubtract) => {
                        Action::Volume(self.preferences.volume - 0.1)
                    }
                    PhysicalKey::Code(KeyCode::Equal | KeyCode::NumpadAdd) => {
                        Action::Volume(self.preferences.volume + 0.1)
                    }
                    _ => return,
                };
                self.action(action, event_loop);
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.frame() {
                    self.fatal = Some(error);
                    self.finish(event_loop);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self
            .args
            .duration
            .is_some_and(|duration| self.start.elapsed() >= Duration::from_secs(duration.into()))
        {
            self.finish(event_loop);
            return;
        }
        if let Some(window) = &self.window {
            let now = Instant::now();
            if now >= self.next {
                window.request_redraw();
                let fps = if self.paused {
                    12
                } else if self.quality() == Quality::Low {
                    30
                } else {
                    60
                };
                self.next = now + Duration::from_secs_f32(1. / fps as f32);
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        if let Err(error) = self.checkpoint() {
            self.fatal.get_or_insert(error);
        }
    }
}
