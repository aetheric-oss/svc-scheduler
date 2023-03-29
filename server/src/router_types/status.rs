//! Definition for the [`Status`] type, implemented by an enum.
use serde::{Deserialize, Serialize};

/// Represent the operating status of a [`super::node::Node`].
#[derive(Debug, PartialEq, Hash, Eq, Copy, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum Status {
    /// Indicate that the node is currently operating.
    Ok,
    /// Indicate that the node is currently down.
    Closed,
}
