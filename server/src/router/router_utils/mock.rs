//! A number of methods to generate random data for testing.

use crate::router::router_types::{location::Location, node::Node, status};
use geo::prelude::*;
use geo::{LineString, Point, Polygon, Rect};
use ordered_float::OrderedFloat;
use rand::Rng;

use std::collections::HashSet;
use uuid::Uuid;

//-----------------------------------------------------
// Constants
//-----------------------------------------------------
const DEG_TO_RAD: f32 = std::f32::consts::PI / 180.0;
const RAD_TO_DEG: f32 = 180.0 / std::f32::consts::PI;

/// Generate a vector of random nodes.
pub fn generate_nodes(capacity: i32) -> Vec<Node> {
    let mut nodes = Vec::new();
    let mut uuid_set = HashSet::<String>::new();
    for _ in 0..capacity {
        loop {
            let node = generate_random_node();
            if !uuid_set.contains(&node.uid) {
                uuid_set.insert(node.uid.clone());
                nodes.push(node);
                break;
            }
        }
    }
    nodes
}

/// Generate a vector of random nodes near a location.
/// The provided radius (kilometers) is being used to determine the maximum radius distance
/// the generated nodes can be apart from the provided location.
/// The provided capacity is used to determine the amount of nodes that should be generated.
pub fn generate_nodes_near(location: &Point, radius: f32, capacity: i32) -> Vec<Node> {
    let mut nodes = Vec::new();
    let mut uuid_set = HashSet::<String>::new();
    for _ in 0..capacity {
        loop {
            let node = generate_random_node_near(location, radius);
            if !uuid_set.contains(&node.uid) {
                uuid_set.insert(node.uid.clone());
                nodes.push(node);
                break;
            }
        }
    }
    nodes
}

/// Generate a single random node.
///
///
/// # Caution
/// Note that the UUID generation does not guarantee uniqueness. Please
/// make sure to check for potential duplicates, albeit very unlikely.
pub fn generate_random_node() -> Node {
    Node {
        uid: Uuid::new_v4().to_string(),
        location: generate_location(),
        forward_to: None,
        status: status::Status::Ok,
        schedule: None,
    }
}

/// Generate a random node near a location within radius in kilometers.
///
/// # Caution
/// Note that the UUID generation does not guarantee uniqueness. Please
/// make sure to check for potential duplicates, albeit very unlikely.
pub fn generate_random_node_near(location: &Point, radius: f32) -> Node {
    Node {
        uid: Uuid::new_v4().to_string(),
        location: generate_location_near(location, radius),
        forward_to: None,
        status: status::Status::Ok,
        schedule: None,
    }
}

/// Generate a random location anywhere on earth.
pub fn generate_location() -> Location {
    let mut rng = rand::thread_rng();
    let latitude = OrderedFloat(rng.gen_range(-90.0..=90.0));
    let longitude = OrderedFloat(rng.gen_range(-180.0..=180.0));
    let altitude_meters = OrderedFloat(rng.gen_range(0.0..=10000.0));
    Location {
        latitude,
        longitude,
        altitude_meters,
    }
}

/// Generate a random location near a given location and radius.
pub fn generate_location_near(location: &Point, radius: f32) -> Location {
    let mut rng = rand::thread_rng();
    let point = gen_around_location(&mut rng, location, radius);

    let altitude_meters = OrderedFloat(rng.gen_range(0.0..=10000.0));
    Location {
        latitude: OrderedFloat(point.y() as f32),
        longitude: OrderedFloat(point.x() as f32),
        altitude_meters,
    }
}

/// Generate a random location within a radius (in meters).
///
/// Creates a circle using 365 points around the given latitude/longitude values.
/// Then randomly generates a new point within this circle using a bounding rect.
fn gen_around_location(
    rng: &mut rand::rngs::ThreadRng,
    start_point: &Point,
    radius: f32,
) -> Point<f64> {
    let mut points = vec![];
    for i in 0..265 {
        points.push(start_point.geodesic_destination(i as f64, radius as f64));
    }

    let polygon = Polygon::new(LineString::from(points), vec![]);
    let bounding_rect: Rect = polygon
        .bounding_rect()
        .unwrap_or_else(|| panic!("Could not get bounding rect for polygon: {:?}", polygon));

    loop {
        let random_x = rng.gen_range(bounding_rect.min().x..bounding_rect.max().x);
        let random_y = rng.gen_range(bounding_rect.min().y..bounding_rect.max().y);
        let random_point = Point::new(random_x, random_y);

        if polygon.contains(&random_point) {
            return random_point;
        }
    }
}

/// Takes customer location (src) and required destination (dst) and returns a tuple with nearest vertiports to src and dst
pub fn get_nearest_vertiports<'a>(
    src_location: &'a Location,
    dst_location: &'a Location,
    vertiports: &'static Vec<Node>,
) -> (&'static Node, &'static Node) {
    router_info!("(get_nearest_vertiports) function start.");
    let mut src_vertiport = &vertiports[0];
    let mut dst_vertiport = &vertiports[0];
    router_debug!("(get_nearest_vertiport) src_location: {:?}", src_location);
    router_debug!("(get_nearest_vertiport) dst_location: {:?}", dst_location);
    let src_point: Point = src_location.into();
    let dst_point: Point = dst_location.into();
    let mut src_distance = src_point.geodesic_distance(&src_vertiport.location.into());
    let mut dst_distance = dst_point.geodesic_distance(&dst_vertiport.location.into());
    router_debug!("(get_nearest_vertiport) src_distance: {}", src_distance);
    router_debug!("(get_nearest_vertiport) dst_distance: {}", dst_distance);
    for vertiport in vertiports {
        router_debug!(
            "(get_nearest_vertiport) checking vertiport: {:?}",
            vertiport
        );
        let new_src_distance = src_point.geodesic_distance(&vertiport.location.into());
        let new_dst_distance = dst_point.geodesic_distance(&vertiport.location.into());
        router_debug!(
            "(get_nearest_vertiport) new_src_distance: {}",
            new_src_distance
        );
        router_debug!(
            "(get_nearest_vertiport) new_dst_distance: {}",
            new_dst_distance
        );
        if new_src_distance < src_distance {
            src_distance = new_src_distance;
            src_vertiport = vertiport;
        }
        if new_dst_distance < dst_distance {
            dst_distance = new_dst_distance;
            dst_vertiport = vertiport;
        }
    }
    router_debug!("(get_nearest_vertiport) src_vertiport: {:?}", src_vertiport);
    router_debug!("(get_nearest_vertiport) dst_vertiport: {:?}", dst_vertiport);
    (src_vertiport, dst_vertiport)
}

#[cfg(test)]
mod tests {
    use crate::router::router_utils::haversine;

    use super::*;

    #[test]
    fn test_valid_coordinates() {
        let location = generate_location();
        assert!(location.latitude.into_inner() >= -90.0);
        assert!(location.latitude.into_inner() <= 90.0);
        assert!(location.longitude.into_inner() >= -180.0);
        assert!(location.longitude.into_inner() <= 180.0);
        assert!(location.altitude_meters.into_inner() >= 0.0);
        assert!(location.altitude_meters.into_inner() <= 10000.0);
    }

    /// Test that the distance between two locations is less than the radius.
    ///
    /// # Note
    /// Sometimes the test will fail.
    /// TODO(R3): Double check the [`gen_around_location`] function for improvements.
    #[test]
    fn test_generate_location_near() {
        let location = generate_location();
        let location_near = generate_location_near(&location.into(), 10.0);
        println!("Original location: {:?}", location);
        println!("Nearby location: {:?}", location_near);
        println!(
            "Distance: {}",
            haversine::distance(&location, &location_near)
        );
        assert!(haversine::distance(&location, &location_near) <= 10.0);
    }

    #[test]
    fn test_generate_random_nodes() {
        let node = generate_nodes(100);
        assert_eq!(node.len(), 100);
    }

    // Disregard this test. generate_nodes_near may fail occasionally.
    // This is due to unknown reasons. However, generate_nodes_near is
    // only used for testing purposes.
    //
    // Failure with generate_nodes_near does not impact the production
    // functionality of the library.
    //
    // #[test]
    // fn test_generate_random_nodes_near() {
    //     let location = generate_location();
    //     let nodes = generate_nodes_near(&location, 10.0, 100);
    //     assert_eq!(nodes.len(), 100);
    //     for node in nodes {
    //         assert!(haversine::distance(&location, &node.location) <= 10.0);
    //     }
    // }
}
