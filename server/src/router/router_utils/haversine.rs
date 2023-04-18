//! Implementation of the Haversine formula for calculating the distance
//! between two points on a sphere.
//!
//! See [Wikipedia](https://en.wikipedia.org/wiki/Haversine_formula) for
//! more.
//!
//! **Distance is returned in kilometers**.

use crate::router::router_types::location::Location;

/// Calculate the distance between two points on a sphere.
///
/// # Notes
/// The current formula does ***not*** take into account the altitude of the
/// points.
///
/// Float 32 values are used to achieve a 5-decimal precision (0.00001),
/// which narrows the error margin to a meter.
pub fn distance(start: &Location, end: &Location) -> f32 {
    // km in radians
    let kilometers: f32 = 6371.0;

    let d_lat: f32 = (end.latitude.into_inner() - start.latitude.into_inner()).to_radians();
    let d_lon: f32 = (end.longitude.into_inner() - start.longitude.into_inner()).to_radians();
    let lat1: f32 = (start.latitude.into_inner()).to_radians();
    let lat2: f32 = (end.latitude.into_inner()).to_radians();

    let a: f32 = ((d_lat / 2.0).sin()) * ((d_lat / 2.0).sin())
        + ((d_lon / 2.0).sin()) * ((d_lon / 2.0).sin()) * (lat1.cos()) * (lat2.cos());
    let c: f32 = 2.0 * ((a.sqrt()).atan2((1.0 - a).sqrt()));

    kilometers * c
}

#[cfg(test)]
pub mod haversine_test {
    use super::*;
    use ordered_float::OrderedFloat;

    #[test]
    fn haversine_distance_in_kilometers() {
        let start = Location {
            latitude: OrderedFloat(38.898556),
            longitude: OrderedFloat(-77.037852),
            altitude_meters: OrderedFloat(0.0),
        };
        let end = Location {
            latitude: OrderedFloat(38.897147),
            longitude: OrderedFloat(-77.043934),
            altitude_meters: OrderedFloat(0.0),
        };
        assert_eq!(0.5496312, distance(&start, &end));
    }
}
