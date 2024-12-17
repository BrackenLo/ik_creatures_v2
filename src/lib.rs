use core::f32;
use std::time::Duration;

use ik::NodeManager;
use renderer::{CircleInstance, Renderer};
use roots_core::{
    common::{
        input::{self, Input, MouseInput},
        Size, Time,
    },
    runner::{
        prelude::{KeyCode, LevelFilter, MouseButton},
        window::Window,
        winit::event_loop::ControlFlow,
        RunnerState, WindowInputEvent,
    },
};
use substates::SubState;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

mod ik;
mod polygon_manager;
mod renderer;
mod substates;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn run() {
    roots_core::runner::Runner::<State>::run(Some(&[
        ("wgpu", LevelFilter::Warn),
        ("roots_", LevelFilter::Trace),
        ("ik_creatures_v2", LevelFilter::Trace),
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
        self.window_size = new_size;
    }

    fn tick(&mut self, event_loop: &roots_core::runner::prelude::ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::wait_duration(self.fps));

        self.update();
        self.render();
    }
}

pub struct State {
    window: Window,
    renderer: Renderer,
    time: Time,
    fps: Duration,
    window_size: Size<u32>,

    keys: Input<KeyCode>,
    mouse_buttons: Input<MouseButton>,
    mouse_input: MouseInput,

    node_manager: NodeManager,
    substate: SubState,
}

impl State {
    fn new(window: Window) -> Self {
        let renderer = Renderer::new(&window);
        let window_size = window.size();

        let mut node_manager = NodeManager::new();

        let substate = SubState::new_ik(&mut node_manager);

        Self {
            window,
            renderer,
            time: Default::default(),
            #[cfg(target_arch = "wasm32")]
            fps: Duration::from_secs_f32(1. / 30.),
            #[cfg(not(target_arch = "wasm32"))]
            fps: Duration::from_secs_f32(1. / 60.),
            window_size,

            keys: Default::default(),
            mouse_buttons: Default::default(),
            mouse_input: Default::default(),

            node_manager,
            substate,
        }
    }

    fn update(&mut self) {
        roots_core::common::tick_time(&mut self.time);

        if self.keys.just_pressed(KeyCode::Space) {
            self.change_state();
        }

        if self.keys.just_pressed(KeyCode::Digit1) {
            self.renderer.render_circles = !self.renderer.render_circles;
            if !self.renderer.render_circles {
                self.renderer.render_polygons = true;
            }
        }

        if self.keys.just_pressed(KeyCode::Digit2) {
            self.renderer.render_polygons = !self.renderer.render_polygons;
            if !self.renderer.render_polygons {
                self.renderer.render_circles = true;
            }
        }

        // Change from winit coordinates (winit 0,0 starts top left) to camera coords (0, 0) screen centre
        let mouse_pos = glam::vec2(
            self.mouse_input.position().x,
            self.window_size.height as f32 - self.mouse_input.position().y,
        ) - glam::vec2(
            self.window_size.width as f32,
            self.window_size.height as f32,
        ) / 2.;

        self.substate
            .update(&self.time, &mut self.node_manager, mouse_pos);

        // Render all nodes
        self.node_manager.get_values().into_iter().for_each(|node| {
            self.renderer
                .circle_pipeline
                .prep_circle(CircleInstance::new(node.pos, node.radius).hollow());
        });

        self.substate
            .render(&mut self.node_manager, &mut self.renderer, mouse_pos);

        // Input management
        input::reset_input(&mut self.keys);
        input::reset_input(&mut self.mouse_buttons);
        input::reset_mouse_input(&mut self.mouse_input);
    }

    fn render(&mut self) {
        self.renderer.prep();
        self.renderer.render();
    }

    fn change_state(&mut self) {
        self.node_manager = NodeManager::new();

        match self.substate {
            SubState::IK(_) => {
                self.substate = SubState::new_fk(&mut self.node_manager, &mut self.renderer);
            }
            SubState::FK(_) => {
                self.substate = SubState::new_creature(&mut self.node_manager, &mut self.renderer);
            }
            SubState::Creature(_) => {
                self.substate = SubState::new_bridge(&mut self.node_manager);
            }
            SubState::Bridge(_) => {
                self.substate = SubState::new_ik(&mut self.node_manager);
            }
        }
    }
}
