//! Fleet Routing Algorithm Library.
//! Handles routing and path-finding tasks.
#[macro_use]
extern crate log;

mod types {
    pub mod edge;
    pub mod location;
    pub mod node;
    pub mod router;
    pub mod status;
}

mod utils {
    pub mod generator;
    pub mod graph;
    pub mod haversine;
    pub mod router_state;
    pub mod schedule;
}

pub use types::*;
pub use utils::*;
