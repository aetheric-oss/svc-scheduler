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
) -> Result<Vec<(GeoLineString, f64)>, BestPathError> {
    let mut paths = match clients.gis.best_path(request.clone()).await {
        Ok(response) => response.into_inner().paths,
        Err(e) => {
            router_error!("(best_path) Failed to get best path: {e}");
            return Err(BestPathError::ClientError);
        }
    };

    if paths.is_empty() {
        router_error!("(best_path) No path found.");
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
    router_debug!("(best_path) svc-gis paths: {:?}", paths);

    // convert segments to GeoLineString
    let mut result: Vec<(GeoLineString, f64)> = vec![];
    for path in paths {
        let Ok(points) = path
            .path
            .into_iter()
            .map(|node| {
                let geom = match node.geom {
                    Some(geom) => geom,
                    None => {
                        router_error!("(best_path) No geometry found for node: {:#?}", node);
                        return Err(BestPathError::NoPathFound);
                    }
                };

                Ok(GeoPoint {
                    latitude: geom.latitude,
                    longitude: geom.longitude,
                })
            })
            .collect::<Result<Vec<GeoPoint>, BestPathError>>()
        else {
            continue;
        };

        let linestring = GeoLineString { points };
        result.push((linestring, path.distance_meters.into()));
    }

    Ok(result)
}
