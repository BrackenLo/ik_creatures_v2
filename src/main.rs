use std::time::Duration;

use ik::{ForwardKinematic, InverseKinematic, Node, NodeManager};
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

mod ik;
mod renderer;

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

        // Change from winit coordinates (winit 0,0 starts top left)
        let mouse_pos = glam::vec2(
            self.mouse_input.position().x,
            self.window_size.height as f32 - self.mouse_input.position().y,
        );

        self.substate.update(&mut self.node_manager, mouse_pos);

        // Render all nodes
        self.node_manager.get_values().into_iter().for_each(|node| {
            self.renderer
                .circle_pipeline
                .prep_circle(CircleInstance::new(node.pos, node.radius).hollow());
        });

        self.substate
            .render(&mut self.node_manager, &mut self.renderer);

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
            SubState::IK { .. } => self.substate = SubState::new_fk(&mut self.node_manager),
            SubState::FK { .. } => self.substate = SubState::new_ik(&mut self.node_manager),
        }
    }
}

pub enum SubState {
    IK {
        ik: InverseKinematic,
    },

    FK {
        fk: ForwardKinematic,
        prev_mouse_pos: glam::Vec2,
        prev_mouse_delta: glam::Vec2,
    },
}

impl SubState {
    pub fn new_ik(node_manager: &mut NodeManager) -> Self {
        let nodes = node_manager.insert_nodes(&[
            Node::unlocked(40.),
            Node::unlocked(40.),
            Node::unlocked(40.),
            Node::unlocked(40.),
            Node::unlocked(40.),
            Node::unlocked(40.),
            Node::unlocked(40.),
            Node::unlocked(40.),
            Node::unlocked(40.),
            Node::unlocked(40.),
            Node::unlocked(40.),
            Node::unlocked(40.),
        ]);

        let ik = InverseKinematic {
            nodes: nodes.clone(),
            anchor: glam::vec2(300., 300.),
            target: glam::vec2(0., 0.),
            cycles: 10,
        };

        Self::IK { ik }
    }

    pub fn new_fk(node_manager: &mut NodeManager) -> Self {
        let nodes = node_manager.insert_nodes(&[
            Node::new(24.),
            Node::new(30.),
            Node::new(30.),
            Node::new(40.),
            Node::new(45.),
            Node::new(50.),
            //
            Node::new(40.),
            //
            Node::new(45.),
            Node::new(50.),
            Node::new(40.),
            Node::new(38.),
            Node::new(30.),
            Node::new(22.),
            Node::new(18.),
            Node::new(10.),
            Node::new(10.),
            Node::new(10.),
            Node::new(10.),
        ]);

        let fk = ForwardKinematic { nodes };

        Self::FK {
            fk,
            prev_mouse_pos: glam::Vec2::ZERO,
            prev_mouse_delta: glam::Vec2::ZERO,
        }
    }

    pub fn update(&mut self, node_manager: &mut NodeManager, mouse_pos: glam::Vec2) {
        match self {
            SubState::IK { ik } => {
                ik.target = mouse_pos;
                ik::fabrik(node_manager, ik);
            }

            SubState::FK {
                fk,
                prev_mouse_pos,
                prev_mouse_delta,
            } => {
                let node = node_manager.get_node_mut(&fk.nodes[0]).unwrap();
                node.pos = mouse_pos;

                let mouse_delta = mouse_pos - *prev_mouse_pos;
                let delta_len = mouse_delta.length();

                if delta_len > 1. {
                    node.rotation = mouse_delta.to_angle();
                    *prev_mouse_pos = mouse_pos;
                    *prev_mouse_delta = mouse_delta;
                }

                ik::process_fk(node_manager, fk);
            }
        }
    }

    pub fn render(&mut self, node_manager: &mut NodeManager, renderer: &mut Renderer) {
        match self {
            SubState::IK { .. } => {}

            SubState::FK {
                fk,
                prev_mouse_pos: _,
                prev_mouse_delta,
            } => {
                let head = node_manager.get_node(&fk.nodes[0]).unwrap();

                renderer.circle_pipeline.prep_circle(
                    CircleInstance::new(
                        head.pos + (prev_mouse_delta.normalize_or_zero() * 20.),
                        5.,
                    )
                    .with_color(glam::vec4(1., 0., 0., 1.)),
                );
            }
        }
    }
}
