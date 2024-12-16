use core::f32;
use std::{
    collections::{hash_map::Values, HashMap},
    f32::consts::{PI, TAU},
};

#[derive(Debug, Clone)]
pub struct Node {
    pub radius: f32,
    pub pos: glam::Vec2,

    // In Radians
    pub rotation: f32,
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
    /// Create new node of size radius with default values
    pub fn new(radius: f32) -> Self {
        Self {
            radius,
            ..Default::default()
        }
    }

    #[inline]
    /// Create a new node with the same min and max rotation.
    /// Rotation should be in radians.
    pub fn locked(radius: f32, rotation: f32) -> Self {
        Self {
            radius,
            max_rotation: rotation,
            min_rotation: rotation,
            ..Default::default()
        }
    }

    #[inline]
    /// Createa new node with 360 degree rotation.
    pub fn unlocked(radius: f32) -> Self {
        Self {
            radius,
            max_rotation: TAU, // 2 pi - 360 degrees
            min_rotation: -TAU,
            ..Default::default()
        }
    }

    #[inline]
    /// Create a new node with min and max rotation in range -angle to angle.
    /// Angle should be in radians.
    pub fn angle(radius: f32, angle: f32) -> Self {
        let angle = angle.abs();

        Self {
            radius,
            max_rotation: angle,
            min_rotation: -angle,
            ..Default::default()
        }
    }

    #[inline]
    /// Create a new node with the given min and max angles.
    /// Angles should be in radians.
    pub fn angles(radius: f32, min: f32, max: f32) -> Self {
        Self {
            radius,
            max_rotation: max,
            min_rotation: min,
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

    pub fn insert_nodes(&mut self, nodes: &[Node]) -> Vec<NodeID> {
        nodes
            .iter()
            .map(|node| self.insert(node.clone()))
            .collect::<Vec<_>>()
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

    pub fn get_nodes_mut(&mut self, node_ids: &[NodeID]) -> Vec<&mut Node> {
        // A little verbose, this next section gets an array of mutable references to our nodes.
        let mut nodes = self
            .nodes
            .iter_mut()
            .filter_map(|(id, node)| match node_ids.contains(id) {
                true => Some((id, node)),
                false => None,
            })
            .collect::<HashMap<_, _>>();

        if nodes.len() < node_ids.len() {
            log::warn!("Invalid ik - some nodes do not exist");
            return Vec::new();
        }

        let nodes = node_ids
            .iter()
            .map(|id| nodes.remove(id).unwrap())
            .collect::<Vec<_>>();

        nodes
    }
}

pub struct ForwardKinematic {
    pub nodes: Vec<NodeID>,
}

pub struct InverseKinematic {
    pub nodes: Vec<NodeID>,
    pub anchor: Option<glam::Vec2>,
    pub target: glam::Vec2,
    pub cycles: usize,
}

fn attach_node_rotations(parent: &Node, child: &mut Node) {
    // Get Direction from parent to child
    let direction_vector = parent.pos - child.pos;
    child.rotation = direction_vector.to_angle();

    // Get the difference in angles between parent and child and clamp if needed
    let rotation_diff = angle_diff(child.rotation, parent.rotation);
    let rotation_diff = rotation_diff.clamp(child.min_rotation, child.max_rotation);
    child.rotation = parent.rotation + rotation_diff;

    child.pos = parent.pos - glam::Vec2::from_angle(child.rotation) * parent.radius;
}

/// Calculate difference between two angles between -π and π.
/// Values passed in and out should be in radians.
#[inline]
pub fn angle_diff(a: f32, b: f32) -> f32 {
    let a = a - b;

    if a > PI {
        a - TAU
    } else if a < -PI {
        a + TAU
    } else {
        a
    }
}

/// Ensure an angle is between -π and π, wrapping around if needed
#[inline]
pub fn _wrap_angle(angle: f32) -> f32 {
    (angle + PI) % TAU - PI
}

pub fn attach_node(parent: &Node, child: &mut Node) {
    let direction_vector = parent.pos - child.pos;
    child.rotation = direction_vector.to_angle();

    child.pos = parent.pos - glam::Vec2::from_angle(child.rotation) * parent.radius;
}

pub fn process_fk(node_manager: &mut NodeManager, fk: &ForwardKinematic) {
    if fk.nodes.len() < 2 {
        return;
    }

    let mut nodes = node_manager.get_nodes_mut(&fk.nodes);

    (1..fk.nodes.len()).for_each(|index| {
        let (a, b) = nodes.split_at_mut(index);

        let parent = &a[index - 1];
        let child = &mut b[0];

        attach_node_rotations(parent, child);
    });
}

/// Forward and backward reaching inverse kinematics
/// Returns true if the end node was able to reach the target
pub fn fabrik(node_manager: &mut NodeManager, ik: &InverseKinematic) -> bool {
    if ik.nodes.len() < 3 {
        log::warn!("Invalid ik node count '{}'", ik.nodes.len());
        return false;
    }

    let mut nodes = node_manager.get_nodes_mut(&ik.nodes);

    let count = nodes.len();
    let last = nodes.len() - 1;

    let initial_rot = nodes[0].rotation;
    let anchor = match ik.anchor {
        Some(anchor) => anchor,
        None => nodes[0].pos,
    };

    for _ in 0..ik.cycles {
        nodes[last].pos = ik.target;

        (0..count - 1).rev().for_each(|index| {
            let (a, b) = nodes.split_at_mut(index + 1);

            let parent = &b[0];
            let child = &mut a[index];

            attach_node(parent, child);
        });

        nodes[0].pos = anchor;
        nodes[0].rotation = initial_rot;

        (1..count).for_each(|index| {
            let (a, b) = nodes.split_at_mut(index);

            let parent = &a[index - 1];
            let child = &mut b[0];

            attach_node_rotations(parent, child);
        });

        if (nodes[last].pos - ik.target).length() < 1. {
            return true;
        }
    }

    false
}
