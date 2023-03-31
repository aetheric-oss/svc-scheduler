//! Router module

/// Types of the router.
pub mod router_types {
    pub mod edge;
    pub mod location;
    pub mod node;
    pub mod router;
    pub mod status;
}

/// Utility functions for the router.
pub mod router_utils {
    pub mod generator;
    pub mod graph;
    pub mod haversine;
    pub mod router_state;
    pub mod schedule;
}
