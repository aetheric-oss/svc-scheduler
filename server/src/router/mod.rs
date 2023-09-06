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
use svc_storage_client_grpc::prelude::*;

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
) -> Result<(GeoLineString, f64), BestPathError> {
    let path = match clients.gis.best_path(request.clone()).await {
        Ok(response) => response.into_inner().segments,
        Err(e) => {
            router_error!("(best_path) Failed to get best path: {e}");
            return Err(BestPathError::ClientError);
        }
    };

    let (last_lat, last_lon) = match path.last() {
        Some(last) => (last.end_latitude, last.end_longitude),
        None => {
            router_error!("(best_path) No path found.");
            return Err(BestPathError::NoPathFound);
        }
    };

    let total_distance_meters = path.iter().map(|x| x.distance_meters as f64).sum();

    router_debug!("(best_path) Path: {:?}", path);
    router_debug!("(best_path) Cost: {:?}", total_distance_meters);

    // convert segments to GeoLineString
    let mut points: Vec<GeoPoint> = path
        .iter()
        .map(|x| GeoPoint {
            latitude: x.start_latitude as f64,
            longitude: x.start_longitude as f64,
        })
        .collect();

    points.push(GeoPoint {
        latitude: last_lat as f64,
        longitude: last_lon as f64,
    });

    let path = GeoLineString { points };

    Ok((path, total_distance_meters))
}
