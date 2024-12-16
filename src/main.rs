use core::f32;
use std::time::Duration;

use ik::{ForwardKinematic, InverseKinematic, Node, NodeID, NodeManager};
use polygon_manager::{CustomPolygonNode, PolygonManager};
use renderer::{CircleInstance, PolygonInstance, Renderer};
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
mod polygon_manager;
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

        if self.keys.just_pressed(KeyCode::Digit1) {
            self.renderer.render_circles = !self.renderer.render_circles;
        }

        if self.keys.just_pressed(KeyCode::Digit2) {
            self.renderer.render_polygons = !self.renderer.render_polygons;
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
            SubState::IK { .. } => {
                self.substate = SubState::new_fk(&mut self.node_manager, &mut self.renderer)
            }
            SubState::FK { .. } => {
                self.substate = SubState::new_creature(&mut self.node_manager, &mut self.renderer)
            }
            SubState::Creature { .. } => {
                self.substate = SubState::new_bridge(&mut self.node_manager)
            }
            SubState::Bridge { .. } => self.substate = SubState::new_ik(&mut self.node_manager),
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

        polygons: PolygonManager,
        instance: PolygonInstance,
    },

    Creature {
        body: ForwardKinematic,
        arm_parent: NodeID,
        arm_right: InverseKinematic,
        arm_left: InverseKinematic,

        prev_mouse_pos: glam::Vec2,
        prev_mouse_delta: glam::Vec2,

        polygons: PolygonManager,
        polygon_body: PolygonInstance,
        polygon_arm_right: PolygonInstance,
        polygon_arm_left: PolygonInstance,
    },

    Bridge {
        ik: InverseKinematic,
        gravity: glam::Vec2,
        gravity_angle: f32,
    },
}

const CREATURE_BODY_COLOR: glam::Vec4 = glam::vec4(0.118, 0.29, 0.082, 1.);
const CREATURE_ARM_COLOR: glam::Vec4 = glam::vec4(0.125, 0.412, 0.067, 1.);

impl SubState {
    pub fn new_ik(node_manager: &mut NodeManager) -> Self {
        let nodes = node_manager.insert_nodes(&[
            Node {
                radius: 40.,
                rotation: -90_f32.to_radians(),
                ..Default::default()
            },
            Node::new(40.),
            Node::new(40.),
            Node::new(40.),
            Node::new(40.),
            Node::new(40.),
            Node::new(40.),
            Node::new(40.),
            Node::new(40.),
            Node::new(40.),
            Node::new(40.),
            Node::new(40.),
        ]);

        let ik = InverseKinematic {
            nodes: nodes.clone(),
            anchor: Some(glam::vec2(0., -100.)),
            target: glam::vec2(0., 0.),
            cycles: 10,
        };

        Self::IK { ik }
    }

    pub fn new_fk(node_manager: &mut NodeManager, renderer: &mut Renderer) -> Self {
        let nodes = node_manager.insert_nodes(&[
            Node::new(50.),
            Node::new(50.),
            Node::new(50.),
            Node::new(50.),
            //
            Node::new(40.),
            Node::new(40.),
            Node::new(40.),
            Node::new(40.),
            Node::new(40.),
            //
            Node::new(30.),
            Node::new(30.),
            Node::new(30.),
            Node::new(30.),
            Node::new(30.),
            Node::new(30.),
            Node::new(30.),
        ]);

        let fk = ForwardKinematic { nodes };

        let polygons = PolygonManager::default();
        let (vertices, indices) =
            polygons.calculate_vertices(&node_manager, &fk.nodes, glam::Vec4::ONE, None, None);

        let instance = renderer
            .polygon_pipeline
            .new_polygon(&renderer.device, &vertices, &indices);

        Self::FK {
            fk,
            prev_mouse_pos: glam::Vec2::ZERO,
            prev_mouse_delta: glam::Vec2::ZERO,
            polygons,
            instance,
        }
    }

    pub fn new_creature(node_manager: &mut NodeManager, renderer: &mut Renderer) -> Self {
        let mut polygons = PolygonManager::default();

        let body_nodes = node_manager.insert_nodes(&[
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

        let arm_parent = body_nodes[5];

        let body = ForwardKinematic { nodes: body_nodes };

        let mut arm_right_nodes = vec![arm_parent];

        arm_right_nodes.extend_from_slice(&node_manager.insert_nodes(&[
            Node::locked(20., 90_f32.to_radians()),
            Node::angle(50., f32::consts::PI),
            Node::angle(50., f32::consts::PI / 1.5),
            Node::angle(50., f32::consts::PI / 1.5),
        ]));

        polygons.with_custom(vec![
            (
                arm_right_nodes[4],
                CustomPolygonNode {
                    radius: 20.,
                    color: glam::vec4(1., 0., 0., 1.),
                },
            ),
            (
                arm_right_nodes[3],
                CustomPolygonNode {
                    radius: 20.,
                    color: glam::vec4(0.569, 0.463, 0.078, 1.),
                },
            ),
            (
                arm_right_nodes[2],
                CustomPolygonNode {
                    radius: 25.,
                    color: CREATURE_ARM_COLOR,
                },
            ),
        ]);

        let arm_right = InverseKinematic {
            nodes: arm_right_nodes,
            anchor: None,
            target: glam::Vec2::ZERO,
            cycles: 40,
        };

        let mut arm_left_nodes = vec![arm_parent];

        arm_left_nodes.extend_from_slice(&node_manager.insert_nodes(&[
            Node::locked(20., -90_f32.to_radians()),
            Node::angle(50., f32::consts::PI),
            Node::angle(50., f32::consts::PI / 1.5),
            Node::angle(50., f32::consts::PI / 1.5),
        ]));

        polygons.with_custom(vec![
            (
                arm_left_nodes[4],
                CustomPolygonNode {
                    radius: 20.,
                    color: glam::vec4(1., 0., 0., 1.),
                },
            ),
            (
                arm_left_nodes[3],
                CustomPolygonNode {
                    radius: 20.,
                    color: glam::vec4(0.569, 0.463, 0.078, 1.),
                },
            ),
            (
                arm_left_nodes[2],
                CustomPolygonNode {
                    radius: 25.,
                    color: CREATURE_ARM_COLOR,
                },
            ),
        ]);

        let arm_left = InverseKinematic {
            nodes: arm_left_nodes,
            anchor: None,
            target: glam::Vec2::ZERO,
            cycles: 10,
        };

        let body_poly_data =
            polygons.calculate_vertices(node_manager, &body.nodes, CREATURE_BODY_COLOR, None, None);
        let arm_right_poly_data = polygons.calculate_vertices(
            node_manager,
            &arm_right.nodes,
            CREATURE_ARM_COLOR,
            None,
            None,
        );
        let arm_left_poly_data = polygons.calculate_vertices(
            node_manager,
            &arm_left.nodes,
            CREATURE_ARM_COLOR,
            None,
            None,
        );

        let polygon_arm_right = renderer.polygon_pipeline.new_polygon(
            &renderer.device,
            &arm_right_poly_data.0,
            &arm_right_poly_data.1,
        );

        let polygon_arm_left = renderer.polygon_pipeline.new_polygon(
            &renderer.device,
            &arm_left_poly_data.0,
            &arm_left_poly_data.1,
        );

        // Create body after arms to draw on top
        let polygon_body = renderer.polygon_pipeline.new_polygon(
            &renderer.device,
            &body_poly_data.0,
            &body_poly_data.1,
        );

        Self::Creature {
            body,
            arm_parent,
            arm_right,
            arm_left,

            prev_mouse_pos: glam::Vec2::ZERO,
            prev_mouse_delta: glam::Vec2::ZERO,

            polygons,
            polygon_body,
            polygon_arm_right,
            polygon_arm_left,
        }
    }

    pub fn new_bridge(node_manager: &mut NodeManager) -> Self {
        let nodes = node_manager.insert_nodes(&[Node::unlocked(20.); 35]);

        let ik = InverseKinematic {
            nodes,
            anchor: Some(glam::vec2(-300., 0.)),
            target: glam::Vec2::ZERO,
            cycles: 10,
        };

        let gravity_angle = -90_f32.to_radians();
        let gravity = glam::Vec2::from_angle(gravity_angle) * 300.;

        Self::Bridge {
            ik,
            gravity,
            gravity_angle,
        }
    }

    pub fn update(&mut self, time: &Time, node_manager: &mut NodeManager, mouse_pos: glam::Vec2) {
        match self {
            SubState::IK { ik } => {
                ik.target = mouse_pos;
                ik::fabrik(node_manager, ik);
            }

            SubState::FK {
                fk,
                prev_mouse_pos,
                prev_mouse_delta,
                ..
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

            SubState::Creature {
                body,
                arm_parent,
                arm_right,
                arm_left,

                prev_mouse_pos,
                prev_mouse_delta,
                ..
            } => {
                let node = node_manager.get_node_mut(&body.nodes[0]).unwrap();
                node.pos = mouse_pos;

                let mouse_delta = mouse_pos - *prev_mouse_pos;
                let delta_len = mouse_delta.length();

                if delta_len > 1. {
                    node.rotation = mouse_delta.to_angle();
                    *prev_mouse_pos = mouse_pos;
                    *prev_mouse_delta = mouse_delta;
                }

                ik::process_fk(node_manager, body);

                let arm_root = node_manager.get_node(arm_parent).unwrap();

                let arm_root_pos = arm_root.pos;
                let arm_root_rot = arm_root.rotation;

                if !ik::fabrik(node_manager, arm_right) {
                    let new_target_angle = arm_root_rot - 50_f32.to_radians();

                    let new_target_dir = glam::Vec2::from_angle(new_target_angle);
                    arm_right.target = arm_root_pos + new_target_dir * 150.;
                }

                if !ik::fabrik(node_manager, arm_left) {
                    let new_target_angle = arm_root_rot + 50_f32.to_radians();

                    let new_target_dir = glam::Vec2::from_angle(new_target_angle);
                    arm_left.target = arm_root_pos + new_target_dir * 150.;
                }
            }

            SubState::Bridge {
                ik,
                gravity,
                gravity_angle,
            } => {
                ik.nodes.iter().skip(1).for_each(|id| {
                    let node = node_manager.get_node_mut(id).unwrap();
                    node.pos += *gravity * time.delta_seconds();
                });

                ik.target = mouse_pos;

                ik::fabrik(node_manager, ik);

                *gravity_angle += 0.5 * time.delta_seconds();
                *gravity = glam::Vec2::from_angle(*gravity_angle) * 300.;
            }
        }
    }

    pub fn render(
        &mut self,
        node_manager: &mut NodeManager,
        renderer: &mut Renderer,
        mouse_pos: glam::Vec2,
    ) {
        match self {
            SubState::IK { .. } => {
                renderer.circle_pipeline.prep_circle(
                    CircleInstance::new(mouse_pos, 5.).with_color(glam::vec4(1., 0., 0., 1.)),
                );
            }

            SubState::FK {
                fk,
                prev_mouse_pos: _,
                prev_mouse_delta,
                polygons,
                instance,
            } => {
                let head = node_manager.get_node(&fk.nodes[0]).unwrap();

                renderer.circle_pipeline.prep_circle(
                    CircleInstance::new(
                        head.pos + (prev_mouse_delta.normalize_or_zero() * 20.),
                        5.,
                    )
                    .with_color(glam::vec4(1., 0., 0., 1.)),
                );

                let (vertices, indices) = polygons.calculate_vertices(
                    &node_manager,
                    &fk.nodes,
                    glam::Vec4::ONE,
                    None,
                    None,
                );

                instance.update(&renderer.device, &renderer.queue, &vertices, &indices);
            }

            SubState::Creature {
                body,
                arm_parent: _,
                arm_right,
                arm_left,

                prev_mouse_pos: _,
                prev_mouse_delta,

                polygons,
                polygon_body,
                polygon_arm_right,
                polygon_arm_left,
            } => {
                let head = node_manager.get_node(&body.nodes[0]).unwrap();

                renderer.circle_pipeline.prep_circle(
                    CircleInstance::new(
                        head.pos + (prev_mouse_delta.normalize_or_zero() * 20.),
                        5.,
                    )
                    .with_color(glam::vec4(1., 0., 0., 1.)),
                );

                renderer.circle_pipeline.prep_circle(
                    CircleInstance::new(arm_right.target, 5.)
                        .with_color(glam::vec4(0., 1., 0., 1.)),
                );

                renderer.circle_pipeline.prep_circle(
                    CircleInstance::new(arm_left.target, 5.).with_color(glam::vec4(0., 1., 0., 1.)),
                );

                let body_poly_data = polygons.calculate_vertices(
                    node_manager,
                    &body.nodes,
                    CREATURE_BODY_COLOR,
                    None,
                    None,
                );
                polygon_body.update(
                    &renderer.device,
                    &renderer.queue,
                    &body_poly_data.0,
                    &body_poly_data.1,
                );

                let arm_right_poly_data = polygons.calculate_vertices(
                    node_manager,
                    &arm_right.nodes[1..],
                    CREATURE_ARM_COLOR,
                    None,
                    None,
                );
                polygon_arm_right.update(
                    &renderer.device,
                    &renderer.queue,
                    &arm_right_poly_data.0,
                    &arm_right_poly_data.1,
                );

                let arm_left_poly_data = polygons.calculate_vertices(
                    node_manager,
                    &arm_left.nodes[1..],
                    CREATURE_ARM_COLOR,
                    None,
                    None,
                );
                polygon_arm_left.update(
                    &renderer.device,
                    &renderer.queue,
                    &arm_left_poly_data.0,
                    &arm_left_poly_data.1,
                );
            }

            SubState::Bridge {
                ik: _,
                gravity: _,
                gravity_angle: _,
            } => {
                renderer.circle_pipeline.prep_circle(
                    CircleInstance::new(mouse_pos, 5.).with_color(glam::vec4(1., 0., 0., 1.)),
                );
            }
        }
    }
}
