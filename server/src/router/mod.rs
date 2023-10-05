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
    let mut path = match clients.gis.best_path(request.clone()).await {
        Ok(response) => response.into_inner().segments,
        Err(e) => {
            router_error!("(best_path) Failed to get best path: {e}");
            return Err(BestPathError::ClientError);
        }
    };

    if path.is_empty() {
        router_error!("(best_path) No path found.");
        return Err(BestPathError::NoPathFound);
    }

    path.sort_by(|a, b| a.index.cmp(&b.index));
    router_debug!("(best_path) svc-gis Path: {:?}", path);

    let total_distance_meters = path.iter().map(|x| x.distance_meters as f64).sum();

    // convert segments to GeoLineString
    let points: Vec<GeoPoint> = path
        .into_iter()
        .enumerate()
        .flat_map(|(i, x)| {
            let end = GeoPoint {
                latitude: x.end_latitude as f64,
                longitude: x.end_longitude as f64,
            };

            if i == 0 {
                let start = GeoPoint {
                    latitude: x.start_latitude as f64,
                    longitude: x.start_longitude as f64,
                };

                vec![start, end]
            } else {
                vec![end]
            }
        })
        .collect::<Vec<GeoPoint>>();

    router_debug!("(best_path) Points: {:?}", points);
    router_debug!("(best_path) Cost: {:?}", total_distance_meters);

    let path = GeoLineString { points };
    Ok((path, total_distance_meters))
}
