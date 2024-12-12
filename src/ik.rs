use core::f32;
use std::{
    collections::{hash_map::Values, HashMap},
    f32::consts::TAU,
};

#[derive(Debug)]
pub struct Node {
    pub radius: f32,
    pub pos: glam::Vec2,

    // In Radians
    rotation: f32,
    pub max_rotation: f32,
    pub min_rotation: f32,
}

impl Default for Node {
    #[inline]
    fn default() -> Self {
        Self {
            radius: 80.,
            pos: glam::Vec2::ZERO,
            rotation: 0.,
            max_rotation: Self::DEFAULT_ANGLE,
            min_rotation: -Self::DEFAULT_ANGLE,
        }
    }
}

impl Node {
    const DEFAULT_ANGLE: f32 = 0.6981317; // 40 degrees

    #[inline]
    pub fn new(radius: f32) -> Self {
        Self {
            radius,
            ..Default::default()
        }
    }

    #[inline]
    pub fn locked(radius: f32, rotation: f32) -> Self {
        Self {
            radius,
            max_rotation: rotation.to_radians(),
            min_rotation: rotation.to_radians(),
            ..Default::default()
        }
    }

    #[inline]
    pub fn unlocked(radius: f32) -> Self {
        Self {
            radius,
            max_rotation: TAU, // 2 pi - 360 degrees
            min_rotation: -TAU,
            ..Default::default()
        }
    }

    #[inline]
    pub fn angle(radius: f32, angle: f32) -> Self {
        Self {
            radius,
            max_rotation: angle.to_radians(),
            min_rotation: angle.to_radians(),
            ..Default::default()
        }
    }

    #[inline]
    pub fn angles(radius: f32, min: f32, max: f32) -> Self {
        Self {
            radius,
            max_rotation: max.to_radians(),
            min_rotation: min.to_radians(),
            ..Default::default()
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeID(u32);

pub struct NodeManager {
    current_id: NodeID,
    nodes: HashMap<NodeID, Node>,
}

impl Default for NodeManager {
    #[inline]
    fn default() -> Self {
        Self {
            current_id: NodeID(0),
            nodes: HashMap::default(),
        }
    }
}

impl NodeManager {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn insert(&mut self, node: Node) -> NodeID {
        let id = self.current_id;
        self.current_id.0 += 1;
        self.nodes.insert(id, node);
        id
    }

    #[inline]
    pub fn get_node(&self, id: &NodeID) -> Option<&Node> {
        self.nodes.get(id)
    }

    #[inline]
    pub fn get_node_mut(&mut self, id: &NodeID) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    #[inline]
    pub fn get_values(&self) -> Values<NodeID, Node> {
        self.nodes.values()
    }
}

pub struct ForwardKinematic {
    pub nodes: Vec<NodeID>,
}

pub struct InverseKinematic {
    pub nodes: Vec<NodeID>,
    pub anchor: glam::Vec2,
    pub target: glam::Vec2,
    pub cycles: usize,
}

// fn attach_node(parent: &Node, child: &mut Node) {
//     let direction_vector = parent.pos - child.pos;
//     child.rotation = direction_vector.to_angle();

//     let rotation_diff = (child.rotation - parent.rotation + TAU + PI) % TAU - PI;
//     let rotation_diff = rotation_diff.clamp(child.min_rotation, child.max_rotation);
//     child.rotation = parent.rotation + rotation_diff;

//     child.pos = parent.pos - glam::Vec2::from_angle(child.rotation) * parent.radius;
// }

pub fn attach_node(parent: &Node, child: &mut Node) {
    let direction_vector = parent.pos - child.pos;
    child.rotation = direction_vector.to_angle();

    child.pos = parent.pos - glam::Vec2::from_angle(child.rotation) * parent.radius;
}

// Forward and backward reaching inverse kinematics
pub fn fabrik(node_manager: &mut NodeManager, ik: &InverseKinematic) {
    if ik.nodes.len() < 3 {
        return;
    }

    // A little verbose, this next section gets an array of mutable references to our nodes.
    let mut nodes = node_manager
        .nodes
        .iter_mut()
        .filter_map(|(id, node)| match ik.nodes.contains(id) {
            true => Some((id, node)),
            false => None,
        })
        .collect::<HashMap<_, _>>();

    if nodes.len() < ik.nodes.len() {
        log::warn!("Invalid ik - some nodes do not exist");
        return;
    }

    let mut nodes = ik
        .nodes
        .iter()
        .map(|id| nodes.remove(id).unwrap())
        .collect::<Vec<_>>();

    let count = nodes.len();

    let initial_rot = nodes[0].rotation;

    for _ in 0..ik.cycles {
        nodes[count].pos = ik.target;

        (0..count - 1).rev().for_each(|index| {
            let (a, b) = nodes.split_at_mut(index + 1);

            let parent = &b[0];
            let child = &mut a[index];

            attach_node(parent, child);
        });

        nodes[0].pos = ik.anchor;
        nodes[0].rotation = initial_rot;

        (1..count).for_each(|index| {
            let (a, b) = nodes.split_at_mut(index);

            let parent = &b[index - 1];
            let child = &mut a[0];

            attach_node(parent, child);
        });

        if nodes[count].pos == ik.target {
            return;
        }
    }
}
