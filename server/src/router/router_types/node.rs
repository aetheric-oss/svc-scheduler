//! Struct definitions and implementations for objects that represent
//! vertices in a graph.
//!
//! The most generic form of a vertex is [`Node`]. In the real world,
//! a vertex could be a [`Vertiport`], which includes a cluster of
//! [`Vertipad`]s. Other possibilities such as a rooftop, a container
//! ship, or a farm can also represent and extend `Node`.
//!
//! Since Rust doesn't have a built-in way to represent an interface
//! type, we use an [`AsNode`] trait to achieve the similar effect. So,
//! a function may take an [`AsNode`] parameter and call its
//! [`as_node`](`AsNode::as_node`) method to get a [`Node`] reference.
//!
//! This pattern allows functions to be agnostic of the type of `Node` to
//! accept as argument.
use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};

use super::location;
use super::status;
use crate::router::router_utils::haversine;
use core::hash::Hash;

/// Since Rust doesn't allow for inheritance, we need to use `trait` as
/// a hack to allow passing "Node-like" objects to functions.
pub trait AsNode {
    /// Returns the generic `Node` struct that an object "extends".
    fn as_node(&self) -> &Node;

    /// Returns the identifier of the node.
    fn get_uid(&self) -> String;

    /// Returns the distance between two nodes using the Haversine
    /// formula.
    fn distance_to(&self, other: &dyn AsNode) -> OrderedFloat<f32>;
}

//------------------------------------------------------------------
// Structs and Implementations
//------------------------------------------------------------------

/// Represent a vertex in a graph.
///
/// Since the actual vertex can be any object, a generic struct is
/// needed for the purpose of abstraction and clarity.
#[derive(Debug, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub struct Node {
    /// Typed as a [`String`] to allow for synthetic ids. One purpose of
    /// using a synthetic id is to allow for partitioned indexing on the
    /// database layer to efficiently filter data.
    ///
    /// For example, an uid could be `usa:ny:12345`. This format can be
    /// helpful when a client try to get all nodes in New York from a
    /// database. Otherwise, one would need to loop through all nodes
    /// and filter by location -- this would be a runtime computation
    /// that is expensive enough to impact the user experience.
    pub uid: String,

    /// Denote the geographical position of the node.
    ///
    /// See also [`location::Location`].
    pub location: location::Location,

    /// A node might be unavailable for some reasons. If `forward_to` is
    /// not [`None`], incoming traffic will be forwarded to another
    /// node.
    pub forward_to: Option<Box<Node>>,

    /// Indicate the operation status of a node.
    ///
    /// See also [`status::Status`].
    pub status: status::Status,

    /// calendar of the node as RRule string. (Used for scheduling)
    pub schedule: Option<String>,
}

impl AsNode for Node {
    fn as_node(&self) -> &Node {
        self
    }
    fn get_uid(&self) -> String {
        self.uid.clone()
    }
    fn distance_to(&self, other: &dyn AsNode) -> OrderedFloat<f32> {
        haversine::distance(&self.location, &other.as_node().location).into()
    }
}

/// A vertipad allows for take-offs and landings of a single aircraft.
#[derive(Debug)]
pub struct Vertipad<'a> {
    /// The generic node that this vertipad extends.
    pub node: Node,

    /// FAA regulated pad size.
    pub size_square_meters: OrderedFloat<f32>,

    /// Certain pads may have special purposes. For example, a pad may
    /// be used for medical emergency services.
    ///
    /// TODO(R3): Define a struct for permissions.
    pub permissions: Vec<String>,

    /// If there's no vertiport, then the vertipad itself is the vertiport.
    pub owner_port: Option<Vertiport<'a>>,
}

impl Vertipad<'_> {
    /// Update the size_square_meters field of a vertipad.
    ///
    /// CAUTION: Testing purposes only. Updates should not be done from
    /// the router lib.
    #[allow(dead_code)]
    fn update_size_square_meters(&mut self, new_size: OrderedFloat<f32>) {
        self.size_square_meters = new_size;
    }
}

impl AsNode for Vertipad<'_> {
    fn as_node(&self) -> &Node {
        &self.node
    }

    fn get_uid(&self) -> String {
        self.as_node().uid.clone()
    }

    fn distance_to(&self, other: &dyn AsNode) -> OrderedFloat<f32> {
        haversine::distance(&self.as_node().location, &other.as_node().location).into()
    }
}

/// A vertiport that has a collection of vertipads.
#[derive(Debug)]
pub struct Vertiport<'a> {
    /// The generic node that this vertiport extends.
    pub node: Node,

    /// A vertiport may have multiple vertipads.
    pub vertipads: Vec<&'a Vertipad<'a>>,
}

impl<'a> Vertiport<'a> {
    /// Adds a vertipad to the vertiport.
    #[allow(dead_code)]
    pub fn add_vertipad(&mut self, vertipad: &'a Vertipad) {
        self.vertipads.push(vertipad);
    }
}

impl AsNode for Vertiport<'_> {
    fn as_node(&self) -> &Node {
        &self.node
    }

    fn get_uid(&self) -> String {
        self.as_node().uid.clone()
    }

    fn distance_to(&self, other: &dyn AsNode) -> OrderedFloat<f32> {
        haversine::distance(&self.as_node().location, &other.as_node().location).into()
    }
}

//------------------------------------------------------------------
// Unit Tests
//------------------------------------------------------------------

/// Tests that an extended node type like [`Vertiport`] can be passed
/// in as an [`AsNode`] trait implementation.
#[cfg(test)]
mod node_type_tests {
    use super::*;

    /// Tests that we can make modifications.
    #[test]
    fn test_mutability() {
        let mut vertipad_1 = Vertipad {
            node: Node {
                uid: "vertipad_1".to_string(),
                location: location::Location {
                    longitude: OrderedFloat(-73.935242),
                    latitude: OrderedFloat(40.730610),
                    altitude_meters: OrderedFloat(0.0),
                },
                forward_to: None,
                status: status::Status::Ok,
                schedule: None,
            },
            size_square_meters: OrderedFloat(100.0),
            permissions: vec!["medical".to_string()],
            owner_port: None,
        };
        let vertipad_2 = Vertipad {
            node: Node {
                uid: "vertipad_2".to_string(),
                location: location::Location {
                    longitude: OrderedFloat(-73.935242),
                    latitude: OrderedFloat(40.730610),
                    altitude_meters: OrderedFloat(0.0),
                },
                forward_to: None,
                status: status::Status::Ok,
                schedule: None,
            },
            size_square_meters: OrderedFloat(100.0),
            permissions: vec!["medical".to_string()],
            owner_port: None,
        };
        let vertipad_3 = Vertipad {
            node: Node {
                uid: "vertipad_3".to_string(),
                location: location::Location {
                    longitude: OrderedFloat(-73.935242),
                    latitude: OrderedFloat(40.730610),
                    altitude_meters: OrderedFloat(0.0),
                },
                forward_to: None,
                status: status::Status::Ok,
                schedule: None,
            },
            size_square_meters: OrderedFloat(100.0),
            permissions: vec!["medical".to_string()],
            owner_port: None,
        };
        let mut vertiport = Vertiport {
            node: Node {
                uid: "vertiport_1".to_string(),
                location: location::Location {
                    longitude: OrderedFloat(-73.935242),
                    latitude: OrderedFloat(40.730610),
                    altitude_meters: 0.0.into(),
                },
                forward_to: None,
                status: status::Status::Ok,
                schedule: None,
            },
            vertipads: vec![],
        };

        let vertipad_4 = Vertipad {
            node: Node {
                uid: "vertipad_4".to_string(),
                location: location::Location {
                    longitude: OrderedFloat(-73.935242),
                    latitude: OrderedFloat(40.730610),
                    altitude_meters: 0.0.into(),
                },
                forward_to: None,
                status: status::Status::Ok,
                schedule: None,
            },
            size_square_meters: OrderedFloat(100.0),
            permissions: vec!["medical".to_string()],
            owner_port: None,
        };
        // add all vertipads to the vertiport.
        vertiport.add_vertipad(&vertipad_1);
        vertiport.add_vertipad(&vertipad_2);
        vertiport.add_vertipad(&vertipad_3);
        vertiport.add_vertipad(&vertipad_4);

        // check that the vertiport has all vertipads.
        assert_eq!(vertiport.vertipads.len(), 4);

        // print the uid of each vertipad in the vertiport.
        assert_eq!(vertiport.vertipads[0].node.uid, "vertipad_1".to_string());
        assert_eq!(vertiport.vertipads[1].node.uid, "vertipad_2".to_string());
        assert_eq!(vertiport.vertipads[2].node.uid, "vertipad_3".to_string());
        assert_eq!(vertiport.vertipads[3].node.uid, "vertipad_4".to_string());

        let new_pad_size = 200.0;
        // update the size of vertipad_1.
        vertipad_1.update_size_square_meters(new_pad_size.into());

        // check that the size of vertipad_1 has been updated.
        assert_eq!(vertipad_1.size_square_meters, new_pad_size);
    }

    #[test]
    fn test_get_node_props_from_vertipad() {
        let vertipad = Vertipad {
            node: Node {
                uid: "vertipad_1".to_string(),
                location: location::Location {
                    longitude: OrderedFloat(-73.935242),
                    latitude: OrderedFloat(40.730610),
                    altitude_meters: OrderedFloat(0.0),
                },
                forward_to: None,
                status: status::Status::Ok,
                schedule: None,
            },
            size_square_meters: OrderedFloat(100.0),
            permissions: vec!["public".to_string()],
            owner_port: None,
        };
        assert_eq!(vertipad.get_uid(), "vertipad_1");
    }

    #[test]
    fn test_distance_to() {
        let vertipad_1 = Vertipad {
            node: Node {
                uid: "vertipad_1".to_string(),
                location: location::Location {
                    longitude: OrderedFloat(-73.935242),
                    latitude: OrderedFloat(40.730610),
                    altitude_meters: OrderedFloat(0.0),
                },
                forward_to: None,
                status: status::Status::Ok,
                schedule: None,
            },
            size_square_meters: OrderedFloat(100.0),
            permissions: vec!["public".to_string()],
            owner_port: None,
        };
        let vertipad_2 = Vertipad {
            node: Node {
                uid: "vertipad_2".to_string(),
                location: location::Location {
                    longitude: OrderedFloat(-33.935242),
                    latitude: OrderedFloat(40.730610),
                    altitude_meters: OrderedFloat(0.0),
                },
                forward_to: None,
                status: status::Status::Ok,
                schedule: None,
            },
            size_square_meters: OrderedFloat(100.0),
            permissions: vec!["public".to_string()],
            owner_port: None,
        };
        let vertiport = Vertiport {
            node: Node {
                uid: "vertiport_1".to_string(),
                location: location::Location {
                    longitude: OrderedFloat(-73.935242),
                    latitude: OrderedFloat(40.730610),
                    altitude_meters: 0.0.into(),
                },
                forward_to: None,
                status: status::Status::Ok,
                schedule: None,
            },
            vertipads: vec![],
        };
        assert_eq!(vertiport.distance_to(&vertipad_1), 0.0);
        assert_eq!(vertiport.distance_to(&vertipad_2), 3340.5833);
    }
}
