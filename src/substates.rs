use core::f32;
use std::collections::HashMap;

use roots_core::common::Time;

use crate::{
    ik::{self, ForwardKinematic, InverseKinematic, Node, NodeID, NodeManager},
    polygon_manager::{PolygonManager, PolygonNode},
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
    pub fn new_bridge(node_manager: &mut NodeManager, renderer: &mut Renderer) -> Self {
        Self::Bridge(BridgeSubstate::new(node_manager, renderer))
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
            SubState::Bridge(bridge) => bridge.render(&node_manager, renderer, mouse_pos),
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

    pub fn render(&mut self, node_manager: &NodeManager, renderer: &mut Renderer) {
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
    prev_mouse_pos: glam::Vec2,
    prev_mouse_delta: glam::Vec2,

    polygons: PolygonManager,
    polygon_body: PolygonInstance,

    arm_right: CreatureLimb,
    arm_left: CreatureLimb,
    leg_right: CreatureLimb,
    leg_left: CreatureLimb,
}

pub struct CreatureLimb {
    ik: InverseKinematic,
    polygons: PolygonManager,
    instance: PolygonInstance,
    limb_reach_range: f32,
    limb_reach_angle: f32,
    color: glam::Vec4,
}

impl CreatureLimb {
    pub fn new(
        node_manager: &mut NodeManager,
        renderer: &mut Renderer,
        parent: NodeID,
        nodes: &[Node],
        custom: HashMap<usize, PolygonNode>,
        limb_reach_range: f32,
        limb_reach_angle: f32,
        color: glam::Vec4,
    ) -> Self {
        let mut limb_nodes = vec![parent];

        limb_nodes.extend_from_slice(&node_manager.insert_nodes(nodes));

        let mut polygons = PolygonManager::default();
        let custom = custom
            .into_iter()
            .filter_map(|(index, data)| {
                let node_id = limb_nodes.get(index)?;
                Some((*node_id, data))
            })
            .collect();
        polygons.with_custom(custom);

        let ik = InverseKinematic {
            nodes: limb_nodes,
            anchor: None,
            target: glam::Vec2::ZERO,
            cycles: 10,
        };

        let (vertices, indices) =
            polygons.calculate_vertices(&node_manager, &ik.nodes, color, None, None);
        let instance = renderer
            .polygon_pipeline
            .new_polygon(&renderer.device, &vertices, &indices);

        Self {
            ik,
            polygons,
            instance,
            limb_reach_range,
            limb_reach_angle,
            color,
        }
    }

    pub fn update(&mut self, node_manager: &mut NodeManager) {
        let limb_root = node_manager.get_node(&self.ik.nodes[0]).unwrap();

        let limb_root_pos = limb_root.pos;
        let limb_root_rot = limb_root.rotation;

        if !ik::fabrik(node_manager, &self.ik) {
            let new_target_angle = limb_root_rot + self.limb_reach_angle;

            let new_target_dir = glam::Vec2::from_angle(new_target_angle);
            self.ik.target = limb_root_pos + new_target_dir * self.limb_reach_range;
        }
    }

    pub fn render(&mut self, node_manager: &NodeManager, renderer: &mut Renderer) {
        renderer.circle_pipeline.prep_circle(
            CircleInstance::new(self.ik.target, 5.).with_color(glam::vec4(0., 1., 0., 1.)),
        );

        let (vertices, indices) = self.polygons.calculate_vertices(
            node_manager,
            &self.ik.nodes[1..],
            self.color,
            None,
            None,
        );

        self.instance
            .update(&renderer.device, &renderer.queue, &vertices, &indices);
    }
}

impl CreatureSubstate {
    // const CREATURE_BODY_COLOR: glam::Vec4 = glam::vec4(0.118, 0.29, 0.082, 1.);
    const CREATURE_BODY_COLOR: glam::Vec4 = glam::vec4(0.2, 0.5, 0., 1.);
    const CREATURE_LIMB_COLOR: glam::Vec4 = glam::vec4(0.125, 0.412, 0.067, 1.);

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

        polygons.with_custom(vec![
            (body_nodes[17], PolygonNode::color((0.2, 0.1, 0.0, 1.))),
            (body_nodes[16], PolygonNode::color((0.2, 0.1, 0.0, 1.))),
            (body_nodes[15], PolygonNode::color((0.2, 0.1, 0.0, 1.))),
            (body_nodes[14], PolygonNode::color((0.2, 0.1, 0.0, 1.))),
            (body_nodes[13], PolygonNode::color((0.3, 0.1, 0.0, 1.))),
            (body_nodes[12], PolygonNode::color((0.3, 0.2, 0.0, 1.))),
            (body_nodes[11], PolygonNode::color((0.3, 0.2, 0.0, 1.))),
            (body_nodes[10], PolygonNode::color((0.3, 0.2, 0.0, 1.))),
            (body_nodes[9], PolygonNode::color((0.2, 0.3, 0.0, 1.))),
            (body_nodes[8], PolygonNode::color((0.2, 0.3, 0.0, 1.))),
            (body_nodes[7], PolygonNode::color((0.2, 0.4, 0.0, 1.))),
            (body_nodes[6], PolygonNode::color((0.2, 0.4, 0.0, 1.))),
        ]);

        let arm_parent = body_nodes[5];

        let arm_right = CreatureLimb::new(
            node_manager,
            renderer,
            arm_parent,
            &[
                Node::locked(20., 90_f32.to_radians()),
                Node::angles(50., -50_f32.to_radians(), f32::consts::PI),
                Node::angles(50., -50_f32.to_radians(), f32::consts::PI),
                Node::angles(50., -50_f32.to_radians(), f32::consts::PI),
            ],
            HashMap::from([
                (4, PolygonNode::radius(20.)),
                (3, PolygonNode::radius(20.)),
                (2, PolygonNode::radius(25.)),
            ]),
            150.,
            -50_f32.to_radians(),
            Self::CREATURE_LIMB_COLOR,
        );

        let arm_left = CreatureLimb::new(
            node_manager,
            renderer,
            arm_parent,
            &[
                Node::locked(20., -90_f32.to_radians()),
                Node::angles(50., -f32::consts::PI, 50_f32.to_radians()),
                Node::angles(50., -f32::consts::PI, 50_f32.to_radians()),
                Node::angles(50., -f32::consts::PI, 50_f32.to_radians()),
            ],
            HashMap::from([
                (4, PolygonNode::radius(20.)),
                (3, PolygonNode::radius(20.)),
                (2, PolygonNode::radius(25.)),
            ]),
            150.,
            50_f32.to_radians(),
            Self::CREATURE_LIMB_COLOR,
        );

        let leg_parent = body_nodes[9];

        let leg_right = CreatureLimb::new(
            node_manager,
            renderer,
            leg_parent,
            &[
                Node::locked(20., 90_f32.to_radians()),
                Node::angles(50., -50_f32.to_radians(), f32::consts::PI),
                Node::angles(50., -50_f32.to_radians(), f32::consts::PI),
                Node::angles(50., -50_f32.to_radians(), f32::consts::PI),
            ],
            HashMap::from([
                (4, PolygonNode::radius(20.)),
                (3, PolygonNode::radius(20.)),
                (2, PolygonNode::radius(25.)),
            ]),
            140.,
            -50_f32.to_radians(),
            Self::CREATURE_LIMB_COLOR,
        );

        let leg_left = CreatureLimb::new(
            node_manager,
            renderer,
            leg_parent,
            &[
                Node::locked(20., -90_f32.to_radians()),
                Node::angles(50., -f32::consts::PI, 50_f32.to_radians()),
                Node::angles(50., -f32::consts::PI, 50_f32.to_radians()),
                Node::angles(50., -f32::consts::PI, 50_f32.to_radians()),
            ],
            HashMap::from([
                (4, PolygonNode::radius(20.)),
                (3, PolygonNode::radius(20.)),
                (2, PolygonNode::radius(25.)),
            ]),
            140.,
            50_f32.to_radians(),
            Self::CREATURE_LIMB_COLOR,
        );

        let body = ForwardKinematic { nodes: body_nodes };

        // Create body after arms to draw on top
        let body_poly_data = polygons.calculate_vertices(
            node_manager,
            &body.nodes,
            Self::CREATURE_BODY_COLOR,
            None,
            None,
        );
        let polygon_body = renderer.polygon_pipeline.new_polygon(
            &renderer.device,
            &body_poly_data.0,
            &body_poly_data.1,
        );

        Self {
            body,
            prev_mouse_pos: glam::Vec2::ZERO,
            prev_mouse_delta: glam::Vec2::ZERO,

            polygons,
            polygon_body,
            arm_right,
            arm_left,
            leg_right,
            leg_left,
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

        self.arm_right.update(node_manager);
        self.arm_left.update(node_manager);
        self.leg_right.update(node_manager);
        self.leg_left.update(node_manager);
    }

    pub fn render(&mut self, node_manager: &NodeManager, renderer: &mut Renderer) {
        let head = node_manager.get_node(&self.body.nodes[0]).unwrap();

        renderer.circle_pipeline.prep_circle(
            CircleInstance::new(
                head.pos + (self.prev_mouse_delta.normalize_or_zero() * 20.),
                5.,
            )
            .with_color(glam::vec4(1., 0., 0., 1.)),
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

        self.arm_right.render(node_manager, renderer);
        self.arm_left.render(node_manager, renderer);
        self.leg_right.render(node_manager, renderer);
        self.leg_left.render(node_manager, renderer);
    }
}

pub struct BridgeSubstate {
    ik: InverseKinematic,
    gravity: glam::Vec2,
    gravity_angle: f32,

    instance: PolygonInstance,
}

impl BridgeSubstate {
    pub fn new(node_manager: &mut NodeManager, renderer: &mut Renderer) -> Self {
        let nodes = node_manager.insert_nodes(&[Node::unlocked(20.); 35]);

        let ik = InverseKinematic {
            nodes,
            anchor: Some(glam::vec2(-300., 0.)),
            target: glam::Vec2::ZERO,
            cycles: 10,
        };

        let gravity_angle = -90_f32.to_radians();
        let gravity = glam::Vec2::from_angle(gravity_angle) * 300.;

        let (vertices, indices) = PolygonManager::default().calculate_vertices(
            &node_manager,
            &ik.nodes,
            glam::vec4(0.322, 0.231, 0., 1.),
            None,
            None,
        );

        let instance = renderer
            .polygon_pipeline
            .new_polygon(&renderer.device, &vertices, &indices);

        Self {
            ik,
            gravity,
            gravity_angle,
            instance,
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

    pub fn render(
        &mut self,
        node_manager: &NodeManager,
        renderer: &mut Renderer,
        mouse_pos: glam::Vec2,
    ) {
        renderer
            .circle_pipeline
            .prep_circle(CircleInstance::new(mouse_pos, 5.).with_color(glam::vec4(1., 0., 0., 1.)));

        let (vertices, indices) = PolygonManager::default().calculate_vertices(
            &node_manager,
            &self.ik.nodes[1..],
            glam::vec4(0.349, 0.278, 0.098, 1.),
            None,
            None,
        );

        self.instance
            .update(&renderer.device, &renderer.queue, &vertices, &indices);
    }
}
