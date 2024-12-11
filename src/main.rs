use renderer::Renderer;
use roots_core::{
    common::{
        input::{self, Input, MouseInput},
        Size, Time,
    },
    runner::{
        prelude::{KeyCode, LevelFilter, MouseButton},
        window::Window,
        RunnerState, WindowInputEvent,
    },
};

pub mod renderer;

fn main() {
    println!("Hello, world!");

    roots_core::runner::Runner::<State>::run(Some(&[
        ("wgpu", LevelFilter::Warn),
        ("roots_", LevelFilter::Trace),
    ]));
}

impl RunnerState for State {
    fn new(event_loop: &roots_core::runner::prelude::ActiveEventLoop) -> Self {
        let window = Window::new(event_loop, None);
        Self::new(window)
    }

    fn new_events(
        &mut self,
        _event_loop: &roots_core::runner::prelude::ActiveEventLoop,
        cause: roots_core::runner::prelude::StartCause,
    ) {
        if let roots_core::runner::prelude::StartCause::ResumeTimeReached { .. } = cause {
            self.window.inner().request_redraw();
        }
    }

    fn input_event(&mut self, event: WindowInputEvent) {
        match event {
            WindowInputEvent::KeyInput { key, pressed } => {
                input::process_inputs(&mut self.keys, key, pressed)
            }

            WindowInputEvent::MouseInput { button, pressed } => {
                input::process_inputs(&mut self.mouse_buttons, button, pressed)
            }
            WindowInputEvent::CursorMoved { position } => {
                input::process_mouse_position(&mut self.mouse_input, position)
            }
            WindowInputEvent::MouseWheel { delta } => {
                input::process_mouse_scroll(&mut self.mouse_input, delta)
            }
            WindowInputEvent::MouseMotion { delta } => {
                input::process_mouse_motion(&mut self.mouse_input, delta)
            }

            WindowInputEvent::CursorEntered => {}
            WindowInputEvent::CursorLeft => {}
        }
    }

    fn resized(&mut self, new_size: Size<u32>) {
        self.renderer.resize(new_size);
    }

    fn tick(&mut self, _: &roots_core::runner::prelude::ActiveEventLoop) {
        self.update();
    }
}

pub struct State {
    window: Window,
    renderer: Renderer,
    time: Time,

    keys: Input<KeyCode>,
    mouse_buttons: Input<MouseButton>,
    mouse_input: MouseInput,
}

impl State {
    fn new(window: Window) -> Self {
        let renderer = Renderer::new(&window);

        Self {
            window,
            renderer,
            time: Default::default(),

            keys: Default::default(),
            mouse_buttons: Default::default(),
            mouse_input: Default::default(),
        }
    }

    fn update(&mut self) {
        roots_core::common::tick_time(&mut self.time);

        self.renderer.prep();
        self.renderer.render();
    }
}
