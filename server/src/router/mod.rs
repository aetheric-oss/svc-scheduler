//! Router module

#[macro_use]
pub mod macros;
pub mod flight_plan;
pub mod itinerary;
pub mod schedule;
pub mod vehicle;
pub mod vertiport;

use crate::grpc::client::GrpcClients;
use svc_gis_client_grpc::prelude::{gis::*, *};

pub enum BestPathError {
    ClientError,
    NoPathFound,
}

impl std::fmt::Display for BestPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            BestPathError::ClientError => write!(f, "Client error"),
            BestPathError::NoPathFound => write!(f, "No path found"),
        }
    }
}

/// Get the best path between two vertiports or a between an aircraft and a vertiport
///  and the total length of the path in meters.
pub async fn best_path(
    request: &BestPathRequest,
    clients: &GrpcClients,
) -> Result<Vec<(Vec<PathNode>, f64)>, BestPathError> {
    let mut paths = clients
        .gis
        .best_path(request.clone())
        .await
        .map_err(|e| {
            router_error!("Failed to get best path: {e}");
            BestPathError::ClientError
        })?
        .into_inner()
        .paths;

    if paths.is_empty() {
        router_error!("No path found.");
        return Err(BestPathError::NoPathFound);
    }

    paths.sort_by(|a, b| {
        if a.distance_meters == b.distance_meters {
            std::cmp::Ordering::Equal
        } else if a.distance_meters < b.distance_meters {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });
    router_debug!("svc-gis paths: {:?}", paths);

    // convert segments to GeoLineString
    let result: Vec<(Vec<PathNode>, f64)> = paths
        .into_iter()
        .map(|path| (path.path, path.distance_meters.into()))
        .collect();

    Ok(result)
}
