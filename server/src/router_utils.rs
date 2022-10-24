use once_cell::sync::OnceCell;
use router::{
    generator::generate_nodes_near,
    haversine,
    location::Location,
    node::Node,
    router::engine::{Algorithm, Router},
};

use ordered_float::OrderedFloat;

pub struct NearbyLocationQuery {
    pub location: Location,
    pub radius: f32,
    pub capacity: i32,
}

pub struct RouteQuery {
    pub aircraft: Aircraft,
    pub from: &'static Node,
    pub to: &'static Node,
}

pub enum Aircraft {
    Cargo,
}

static NODES: OnceCell<Vec<Node>> = OnceCell::new();
static ARROW_CARGO_ROUTER: OnceCell<Router> = OnceCell::new();

static ARROW_CARGO_CONSTRAINT: f32 = 75.0;

pub static SAN_FRANCISCO: Location = Location {
    latitude: OrderedFloat(37.7749),
    longitude: OrderedFloat(-122.4194),
    altitude_meters: OrderedFloat(0.0),
};

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
