use once_cell::sync::OnceCell;
use ordered_float::OrderedFloat;
use router::{
    generator::generate_nodes_near,
    haversine,
    location::Location,
    node::Node,
    router::engine::{Algorithm, Router},
};

/// Query struct for generating nodes near a location.
#[derive(Debug, Copy, Clone)]
pub struct NearbyLocationQuery {
    ///location
    pub location: Location,
    ///radius
    pub radius: f32,
    ///capacity
    pub capacity: i32,
}

/// Query struct to find a route between two nodes
#[derive(Debug, Copy, Clone)]
pub struct RouteQuery {
    ///aircraft
    pub aircraft: Aircraft,
    ///from
    pub from: &'static Node,
    ///to
    pub to: &'static Node,
}

/// Enum with all Aircraft types
#[derive(Debug, Copy, Clone)]
pub enum Aircraft {
    ///Cargo aircraft
    Cargo,
}
/// List of vertiport nodes for routing
pub static NODES: OnceCell<Vec<Node>> = OnceCell::new();
/// Cargo router
pub static ARROW_CARGO_ROUTER: OnceCell<Router> = OnceCell::new();

static ARROW_CARGO_CONSTRAINT: f32 = 75.0;
/// SF central location
pub static SAN_FRANCISCO: Location = Location {
    latitude: OrderedFloat(37.7749),
    longitude: OrderedFloat(-122.4194),
    altitude_meters: OrderedFloat(0.0),
};

/// Time to block vertiport for cargo loading and takeoff
pub const LOADING_AND_TAKEOFF_TIME_MIN: f32 = 10.0;
/// Time to block vertiport for cargo unloading and landing
pub const LANDING_AND_UNLOADING_TIME_MIN: f32 = 10.0;

/// Estimates the time needed to travel between two locations including loading and unloading
/// Estimate should be rather generous to block resources instead of potentially overloading them
pub fn estimate_flight_time_minutes(distance_km: f32, aircraft: Aircraft) -> f32 {
    const AVG_SPEED_KMH: f32 = 60.0;
    match aircraft {
        Aircraft::Cargo => {
            LOADING_AND_TAKEOFF_TIME_MIN
                + distance_km / AVG_SPEED_KMH * 60.0
                + LANDING_AND_UNLOADING_TIME_MIN
        }
    }
}

/// Takes customer location (src) and required destination (dst) and returns a tuple with nearest vertiports to src and dst
pub fn get_nearest_vertiports<'a>(
    src_location: &'a Location,
    dst_location: &'a Location,
    vertiports: &'static Vec<Node>,
) -> (&'static Node, &'static Node) {
    let mut src_vertiport = &vertiports[0];
    let mut dst_vertiport = &vertiports[0];
    let mut src_distance = haversine::distance(src_location, &src_vertiport.location);
    let mut dst_distance = haversine::distance(dst_location, &dst_vertiport.location);
    for vertiport in vertiports {
        let new_src_distance = haversine::distance(src_location, &vertiport.location);
        let new_dst_distance = haversine::distance(dst_location, &vertiport.location);
        if new_src_distance < src_distance {
            src_distance = new_src_distance;
            src_vertiport = vertiport;
        }
        if new_dst_distance < dst_distance {
            dst_distance = new_dst_distance;
            dst_vertiport = vertiport;
        }
    }
    (src_vertiport, dst_vertiport)
}

/// Returns a list of nodes near the given location
pub fn get_nearby_nodes(query: NearbyLocationQuery) -> &'static Vec<Node> {
    NODES
        .set(generate_nodes_near(
            &query.location,
            query.radius,
            query.capacity,
        ))
        .expect("Failed to generate nodes");
    return NODES.get().unwrap();
}

/// Get route
pub fn get_route(req: RouteQuery) -> Result<(Vec<Location>, f32), &'static str> {
    let RouteQuery {
        from,
        to,
        aircraft: _,
    } = req;

    if ARROW_CARGO_ROUTER.get().is_none() {
        return Err("Arrow XL router not initialized. Try to initialize it first.");
    }
    let (cost, path) = ARROW_CARGO_ROUTER
        .get()
        .as_ref()
        .unwrap()
        .find_shortest_path(from, to, Algorithm::Dijkstra, None);
    let locations = path
        .iter()
        .map(|node_idx| {
            ARROW_CARGO_ROUTER
                .get()
                .as_ref()
                .unwrap()
                .get_node_by_id(*node_idx)
                .unwrap()
                .location
        })
        .collect::<Vec<Location>>();
    Ok((locations, cost))
}

/// Initializes the router for the given aircraft
pub fn init_router() -> &'static str {
    if NODES.get().is_none() {
        return "Nodes not initialized. Try to get some nodes first.";
    }
    if ARROW_CARGO_ROUTER.get().is_some() {
        return "Router already initialized. Try to use the router instead of initializing it.";
    }
    ARROW_CARGO_ROUTER
        .set(Router::new(
            NODES.get().as_ref().unwrap(),
            ARROW_CARGO_CONSTRAINT,
            |from, to| haversine::distance(&from.as_node().location, &to.as_node().location),
            |from, to| haversine::distance(&from.as_node().location, &to.as_node().location),
        ))
        .expect("Failed to initialize router");
    "Arrow Cargo router initialized."
}

#[cfg(test)]
mod calendar_tests {
    use super::{
        get_nearby_nodes, get_nearest_vertiports, get_route, init_router, Aircraft,
        NearbyLocationQuery, RouteQuery, SAN_FRANCISCO,
    };
    use ordered_float::OrderedFloat;
    use router::location::Location;

    #[test]
    fn test_router() {
        let nodes = get_nearby_nodes(NearbyLocationQuery {
            location: SAN_FRANCISCO,
            radius: 25.0,
            capacity: 20,
        });

        //println!("nodes: {:?}", nodes);
        let init_res = init_router();
        println!("init_res: {:?}", init_res);
        let src_location = Location {
            latitude: OrderedFloat(37.52123),
            longitude: OrderedFloat(-122.50892),
            altitude_meters: OrderedFloat(20.0),
        };
        let dst_location = Location {
            latitude: OrderedFloat(37.81032),
            longitude: OrderedFloat(-122.28432),
            altitude_meters: OrderedFloat(20.0),
        };
        let (src, dst) = get_nearest_vertiports(&src_location, &dst_location, nodes);
        println!("src: {:?}, dst: {:?}", src.location, dst.location);
        let (route, cost) = get_route(RouteQuery {
            from: src,
            to: dst,
            aircraft: Aircraft::Cargo,
        })
        .unwrap();
        println!("route: {:?}", route);
        assert!(route.len() > 0, "Route should not be empty");
        assert!(cost > 0.0, "Cost should be greater than 0");
    }
}
