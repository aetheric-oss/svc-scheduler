//! Helper functions for working with graphs.

use ordered_float::OrderedFloat;

use crate::router::router_types::{edge::Edge, node::AsNode};

/// Build edges among nodes.
///
/// The function will try to connect every node to every other node.
/// However, constraints can be added to the graph to prevent ineligible
/// nodes from being connected.
///
/// For example, if the constraint represents the max travel distance of
/// an aircraft, we only want to connect nodes that are within the max
/// travel distance. A constraint function is also needed to determine
/// if a connection is valid.
///
/// # Arguments
/// * `nodes` - A vector of nodes.
/// * `constraint` - Only nodes within a constraint can be connected.
/// * `constraint_function` - A function that takes two nodes and
///   returns a float to compare against `constraint`.
/// * `cost_function` - A function that computes the "weight" between
///   two nodes.
///
/// # Returns
/// A vector of edges in the format of (from_node, to_node, weight).
///
/// # Time Complexity
/// *O*(*n^2*) at worst if the constraint is not met for all nodes.
pub fn build_edges(
    nodes: &[impl AsNode],
    constraint: f32,
    constraint_function: fn(&dyn AsNode, &dyn AsNode) -> f32,
    cost_function: fn(&dyn AsNode, &dyn AsNode) -> f32,
) -> Vec<Edge> {
    let mut edges = Vec::new();
    for from in nodes {
        for to in nodes {
            if from.as_node() != to.as_node()
                && constraint_function(from.as_node(), to.as_node()) <= constraint
            {
                let cost = cost_function(from.as_node(), to.as_node());
                edges.push(Edge {
                    from: from.as_node(),
                    to: to.as_node(),
                    cost: OrderedFloat(cost),
                });
            }
        }
    }
    edges
}

#[cfg(test)]
mod tests {
    use crate::router::router_utils::{
        generator::{generate_location, generate_nodes_near},
        haversine,
    };

    use super::*;

    #[test]
    fn test_build_edges() {
        let capacity = 1000;
        let location = generate_location();
        let nodes = generate_nodes_near(&location, 1000.0, capacity);

        // set constraint to 2000 so that all nodes should be connected
        let edges = build_edges(
            &nodes,
            2000.0,
            |from, to| haversine::distance(&from.as_node().location, &to.as_node().location),
            |from, to| haversine::distance(&from.as_node().location, &to.as_node().location),
        );

        assert_eq!(edges.len(), nodes.len() * nodes.len() - capacity as usize);
    }
}
