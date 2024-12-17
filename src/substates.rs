use core::f32;

use roots_core::common::Time;

use crate::{
    ik::{self, ForwardKinematic, InverseKinematic, Node, NodeID, NodeManager},
    polygon_manager::{CustomPolygonNode, PolygonManager},
    renderer::{CircleInstance, PolygonInstance, Renderer},
};

pub enum SubState {
    IK(IKSubstate),
    FK(FKSubstate),
    Creature(CreatureSubstate),
    Bridge(BridgeSubstate),
}

impl SubState {
    #[inline]
    pub fn new_ik(node_manager: &mut NodeManager) -> Self {
        Self::IK(IKSubstate::new(node_manager))
    }

    #[inline]
    pub fn new_fk(node_manager: &mut NodeManager, renderer: &mut Renderer) -> Self {
        Self::FK(FKSubstate::new(node_manager, renderer))
    }

    #[inline]
    pub fn new_creature(node_manager: &mut NodeManager, renderer: &mut Renderer) -> Self {
        Self::Creature(CreatureSubstate::new(node_manager, renderer))
    }

    #[inline]
    pub fn new_bridge(node_manager: &mut NodeManager) -> Self {
        Self::Bridge(BridgeSubstate::new(node_manager))
    }

    #[inline]
    pub fn update(&mut self, time: &Time, node_manager: &mut NodeManager, mouse_pos: glam::Vec2) {
        match self {
            SubState::IK(ik) => ik.update(node_manager, mouse_pos),
            SubState::FK(fk) => fk.update(node_manager, mouse_pos),
            SubState::Creature(creature) => creature.update(node_manager, mouse_pos),
            SubState::Bridge(bridge) => bridge.update(time, node_manager, mouse_pos),
        }
    }

    #[inline]
    pub fn render(
        &mut self,
        node_manager: &mut NodeManager,
        renderer: &mut Renderer,
        mouse_pos: glam::Vec2,
    ) {
        match self {
            SubState::IK(ik) => ik.render(renderer, mouse_pos),
            SubState::FK(fk) => fk.render(node_manager, renderer),
            SubState::Creature(creature) => creature.render(node_manager, renderer),
            SubState::Bridge(bridge) => bridge.render(renderer, mouse_pos),
        }
    }
}

pub struct IKSubstate {
    ik: InverseKinematic,
}

impl IKSubstate {
    pub fn new(node_manager: &mut NodeManager) -> Self {
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

        Self { ik }
    }

    pub fn update(&mut self, node_manager: &mut NodeManager, mouse_pos: glam::Vec2) {
        self.ik.target = mouse_pos;
        ik::fabrik(node_manager, &self.ik);
    }

    pub fn render(&mut self, renderer: &mut Renderer, mouse_pos: glam::Vec2) {
        renderer
            .circle_pipeline
            .prep_circle(CircleInstance::new(mouse_pos, 5.).with_color(glam::vec4(1., 0., 0., 1.)));
    }
}

pub struct FKSubstate {
    fk: ForwardKinematic,
    prev_mouse_pos: glam::Vec2,
    prev_mouse_delta: glam::Vec2,

    polygons: PolygonManager,
    instance: PolygonInstance,
}

impl FKSubstate {
    pub fn new(node_manager: &mut NodeManager, renderer: &mut Renderer) -> Self {
        let data = &[
            [Node::new(50.); 4].as_slice(),
            [Node::new(40.); 5].as_slice(),
            [Node::new(30.); 6].as_slice(),
        ]
        .concat();

        let nodes = node_manager.insert_nodes(data);

        let fk = ForwardKinematic { nodes };

        let polygons = PolygonManager::default();
        let (vertices, indices) =
            polygons.calculate_vertices(&node_manager, &fk.nodes, glam::Vec4::ONE, None, None);

        let instance = renderer
            .polygon_pipeline
            .new_polygon(&renderer.device, &vertices, &indices);

        Self {
            fk,
            prev_mouse_pos: glam::Vec2::ZERO,
            prev_mouse_delta: glam::Vec2::ZERO,
            polygons,
            instance,
        }
    }

    pub fn update(&mut self, node_manager: &mut NodeManager, mouse_pos: glam::Vec2) {
        let node = node_manager.get_node_mut(&self.fk.nodes[0]).unwrap();
        node.pos = mouse_pos;

        let mouse_delta = mouse_pos - self.prev_mouse_pos;
        let delta_len = mouse_delta.length();

        if delta_len > 1. {
            node.rotation = mouse_delta.to_angle();
            self.prev_mouse_pos = mouse_pos;
            self.prev_mouse_delta = mouse_delta;
        }

        ik::process_fk(node_manager, &self.fk);
    }

    pub fn render(&mut self, node_manager: &mut NodeManager, renderer: &mut Renderer) {
        let head = node_manager.get_node(&self.fk.nodes[0]).unwrap();

        renderer.circle_pipeline.prep_circle(
            CircleInstance::new(
                head.pos + (self.prev_mouse_delta.normalize_or_zero() * 20.),
                5.,
            )
            .with_color(glam::vec4(1., 0., 0., 1.)),
        );

        let (vertices, indices) = self.polygons.calculate_vertices(
            &node_manager,
            &self.fk.nodes,
            glam::Vec4::ONE,
            None,
            None,
        );

        self.instance
            .update(&renderer.device, &renderer.queue, &vertices, &indices);
    }
}

pub struct CreatureSubstate {
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
}

impl CreatureSubstate {
    const CREATURE_BODY_COLOR: glam::Vec4 = glam::vec4(0.118, 0.29, 0.082, 1.);
    const CREATURE_ARM_COLOR: glam::Vec4 = glam::vec4(0.125, 0.412, 0.067, 1.);

    pub fn new(node_manager: &mut NodeManager, renderer: &mut Renderer) -> Self {
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
                    color: Self::CREATURE_ARM_COLOR,
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
                    color: Self::CREATURE_ARM_COLOR,
                },
            ),
        ]);

        let arm_left = InverseKinematic {
            nodes: arm_left_nodes,
            anchor: None,
            target: glam::Vec2::ZERO,
            cycles: 10,
        };

        let body_poly_data = polygons.calculate_vertices(
            node_manager,
            &body.nodes,
            Self::CREATURE_BODY_COLOR,
            None,
            None,
        );
        let arm_right_poly_data = polygons.calculate_vertices(
            node_manager,
            &arm_right.nodes,
            Self::CREATURE_ARM_COLOR,
            None,
            None,
        );
        let arm_left_poly_data = polygons.calculate_vertices(
            node_manager,
            &arm_left.nodes,
            Self::CREATURE_ARM_COLOR,
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

        Self {
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

    pub fn update(&mut self, node_manager: &mut NodeManager, mouse_pos: glam::Vec2) {
        let node = node_manager.get_node_mut(&self.body.nodes[0]).unwrap();
        node.pos = mouse_pos;

        let mouse_delta = mouse_pos - self.prev_mouse_pos;
        let delta_len = mouse_delta.length();

        if delta_len > 1. {
            node.rotation = mouse_delta.to_angle();
            self.prev_mouse_pos = mouse_pos;
            self.prev_mouse_delta = mouse_delta;
        }

        ik::process_fk(node_manager, &self.body);

        let arm_root = node_manager.get_node(&self.arm_parent).unwrap();

        let arm_root_pos = arm_root.pos;
        let arm_root_rot = arm_root.rotation;

        if !ik::fabrik(node_manager, &self.arm_right) {
            let new_target_angle = arm_root_rot - 50_f32.to_radians();

            let new_target_dir = glam::Vec2::from_angle(new_target_angle);
            self.arm_right.target = arm_root_pos + new_target_dir * 150.;
        }

        if !ik::fabrik(node_manager, &self.arm_left) {
            let new_target_angle = arm_root_rot + 50_f32.to_radians();

            let new_target_dir = glam::Vec2::from_angle(new_target_angle);
            self.arm_left.target = arm_root_pos + new_target_dir * 150.;
        }
    }

    pub fn render(&mut self, node_manager: &mut NodeManager, renderer: &mut Renderer) {
        let head = node_manager.get_node(&self.body.nodes[0]).unwrap();

        renderer.circle_pipeline.prep_circle(
            CircleInstance::new(
                head.pos + (self.prev_mouse_delta.normalize_or_zero() * 20.),
                5.,
            )
            .with_color(glam::vec4(1., 0., 0., 1.)),
        );

        renderer.circle_pipeline.prep_circle(
            CircleInstance::new(self.arm_right.target, 5.).with_color(glam::vec4(0., 1., 0., 1.)),
        );

        renderer.circle_pipeline.prep_circle(
            CircleInstance::new(self.arm_left.target, 5.).with_color(glam::vec4(0., 1., 0., 1.)),
        );

        let body_poly_data = self.polygons.calculate_vertices(
            node_manager,
            &self.body.nodes,
            Self::CREATURE_BODY_COLOR,
            None,
            None,
        );
        self.polygon_body.update(
            &renderer.device,
            &renderer.queue,
            &body_poly_data.0,
            &body_poly_data.1,
        );

        let arm_right_poly_data = self.polygons.calculate_vertices(
            node_manager,
            &self.arm_right.nodes[1..],
            Self::CREATURE_ARM_COLOR,
            None,
            None,
        );
        self.polygon_arm_right.update(
            &renderer.device,
            &renderer.queue,
            &arm_right_poly_data.0,
            &arm_right_poly_data.1,
        );

        let arm_left_poly_data = self.polygons.calculate_vertices(
            node_manager,
            &self.arm_left.nodes[1..],
            Self::CREATURE_ARM_COLOR,
            None,
            None,
        );
        self.polygon_arm_left.update(
            &renderer.device,
            &renderer.queue,
            &arm_left_poly_data.0,
            &arm_left_poly_data.1,
        );
    }
}

pub struct BridgeSubstate {
    ik: InverseKinematic,
    gravity: glam::Vec2,
    gravity_angle: f32,
}

impl BridgeSubstate {
    pub fn new(node_manager: &mut NodeManager) -> Self {
        let nodes = node_manager.insert_nodes(&[Node::unlocked(20.); 35]);

        let ik = InverseKinematic {
            nodes,
            anchor: Some(glam::vec2(-300., 0.)),
            target: glam::Vec2::ZERO,
            cycles: 10,
        };

        let gravity_angle = -90_f32.to_radians();
        let gravity = glam::Vec2::from_angle(gravity_angle) * 300.;

        Self {
            ik,
            gravity,
            gravity_angle,
        }
    }

    pub fn update(&mut self, time: &Time, node_manager: &mut NodeManager, mouse_pos: glam::Vec2) {
        self.ik.nodes.iter().skip(1).for_each(|id| {
            let node = node_manager.get_node_mut(id).unwrap();
            node.pos += self.gravity * time.delta_seconds();
        });

        self.ik.target = mouse_pos;

        ik::fabrik(node_manager, &self.ik);

        self.gravity_angle += 0.5 * time.delta_seconds();
        self.gravity = glam::Vec2::from_angle(self.gravity_angle) * 300.;
    }

    pub fn render(&mut self, renderer: &mut Renderer, mouse_pos: glam::Vec2) {
        renderer
            .circle_pipeline
            .prep_circle(CircleInstance::new(mouse_pos, 5.).with_color(glam::vec4(1., 0., 0., 1.)));
    }
}
