use core::f32;
use std::collections::HashMap;

use crate::{
    ik::{NodeID, NodeManager},
    renderer::PolygonVertex,
};

#[derive(Clone, Copy)]
pub struct PolygonNode {
    pub radius: Option<f32>,
    pub color: Option<glam::Vec4>,
}

impl PolygonNode {
    #[inline]
    pub fn radius(radius: f32) -> Self {
        Self {
            radius: Some(radius),
            color: None,
        }
    }

    #[inline]
    pub fn color(color: impl Into<glam::Vec4>) -> Self {
        Self {
            radius: None,
            color: Some(color.into()),
        }
    }

    #[inline]
    pub fn all(radius: f32, color: impl Into<glam::Vec4>) -> Self {
        Self {
            radius: Some(radius),
            color: Some(color.into()),
        }
    }
}

#[derive(Default)]
pub struct PolygonManager {
    custom_nodes: HashMap<NodeID, PolygonNode>,
}

impl PolygonManager {
    #[inline]
    pub fn with_custom(&mut self, nodes: Vec<(NodeID, PolygonNode)>) {
        nodes.into_iter().for_each(|(id, node)| {
            self.custom_nodes.insert(id, node);
        });
    }

    pub fn calculate_vertices(
        &self,
        node_manager: &NodeManager,
        nodes: &[NodeID],
        color: glam::Vec4,
        start_color: Option<glam::Vec4>,
        end_color: Option<glam::Vec4>,
    ) -> (Vec<PolygonVertex>, Vec<u16>) {
        if nodes.is_empty() {
            panic!("No nodes provided to calculate vertices");
        }

        let start_color = start_color.unwrap_or(color);
        let end_color = end_color.unwrap_or(color);

        let mut vertices = nodes
            .iter()
            .flat_map(|node_id| {
                let node = node_manager.get_node(node_id).unwrap();

                let (radius, color) = match self.custom_nodes.get(node_id) {
                    Some(PolygonNode {
                        radius: custom_radius,
                        color: custom_color,
                    }) => (
                        custom_radius.unwrap_or(node.radius),
                        custom_color.unwrap_or(color),
                    ),

                    None => (node.radius, color),
                };

                [
                    PolygonVertex {
                        pos: glam::Vec2::from_angle(node.rotation - f32::consts::FRAC_PI_2)
                            * radius
                            + node.pos,
                        pad: [0; 2],
                        color,
                    },
                    PolygonVertex {
                        pos: glam::Vec2::from_angle(node.rotation + f32::consts::FRAC_PI_2)
                            * radius
                            + node.pos,
                        pad: [0; 2],
                        color,
                    },
                ]
            })
            .collect::<Vec<_>>();

        let head = node_manager.get_node(&nodes[0]).unwrap();
        vertices.insert(
            0,
            PolygonVertex {
                pos: head.get_relative_point(0.),
                pad: [0; 2],
                color: start_color,
            },
        );

        let tail = node_manager.get_node(nodes.last().unwrap()).unwrap();
        vertices.push(PolygonVertex {
            pos: tail.get_relative_point(f32::consts::PI),
            pad: [0; 2],
            color: end_color,
        });

        let indices = (3..vertices.len())
            .step_by(2)
            .fold(Vec::new(), |mut acc, index| {
                acc.push(index as u16 - 3); // 0
                acc.push(index as u16 - 2); // 1
                acc.push(index as u16 - 1); // 2

                acc.push(index as u16 - 1); // 2
                acc.push(index as u16 - 2); // 1
                acc.push(index as u16); // 3

                acc
            });

        (vertices, indices)
    }
}
