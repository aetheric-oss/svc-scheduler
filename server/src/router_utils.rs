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
    ArrowCargo,
    ArrowXl,
    ArrowInterstate,
}

static mut NODES: Option<Vec<Node>> = None;

static mut ARROW_XL_ROUTER: Option<Router> = None;
static mut ARROW_CARGO_ROUTER: Option<Router> = None;
static mut ARROW_INTERSTATE_ROUTER: Option<Router> = None;

static ARROW_XL_CONSTRAINT: f32 = 25.0;
static ARROW_CARGO_CONSTRAINT: f32 = 75.0;
static ARROW_INTERSTATE_CONSTRAINT: f32 = 100.0;

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
    unsafe {
        ARROW_XL_ROUTER = None;
        ARROW_CARGO_ROUTER = None;
        ARROW_INTERSTATE_ROUTER = None;
        NODES = Some(generate_nodes_near(
            &query.location,
            query.radius,
            query.capacity,
        ));
        return NODES.as_ref().unwrap();
    };
}

/// Get route
pub fn get_route(req: RouteQuery) -> Result<Vec<Location>, &'static str> {
    match req {
        RouteQuery {
            aircraft: Aircraft::ArrowXl,
            from,
            to,
        } => unsafe {
            if ARROW_XL_ROUTER.is_none() {
                return Err("Arrow XL router not initialized. Try to initialize it first.");
            }
            let (_, path) = ARROW_XL_ROUTER.as_ref().unwrap().find_shortest_path(
                from,
                to,
                Algorithm::Dijkstra,
                None,
            );
            let locations = path
                .iter()
                .map(|node_idx| {
                    ARROW_XL_ROUTER
                        .as_ref()
                        .unwrap()
                        .get_node_by_id(*node_idx)
                        .unwrap()
                        .location
                })
                .collect::<Vec<Location>>();
            Ok(locations)
        },
        RouteQuery {
            aircraft: Aircraft::ArrowCargo,
            from,
            to,
        } => unsafe {
            if ARROW_CARGO_ROUTER.is_none() {
                return Err("Arrow XL router not initialized. Try to initialize it first.");
            }
            let (_, path) = ARROW_CARGO_ROUTER.as_ref().unwrap().find_shortest_path(
                from,
                to,
                Algorithm::Dijkstra,
                None,
            );
            let locations = path
                .iter()
                .map(|node_idx| {
                    ARROW_CARGO_ROUTER
                        .as_ref()
                        .unwrap()
                        .get_node_by_id(*node_idx)
                        .unwrap()
                        .location
                })
                .collect::<Vec<Location>>();
            Ok(locations)
        },
        RouteQuery {
            aircraft: Aircraft::ArrowInterstate,
            from,
            to,
        } => unsafe {
            if ARROW_INTERSTATE_ROUTER.is_none() {
                return Err("Arrow XL router not initialized. Try to initialize it first.");
            }
            let (_, path) = ARROW_INTERSTATE_ROUTER
                .as_ref()
                .unwrap()
                .find_shortest_path(from, to, Algorithm::Dijkstra, None);
            let locations = path
                .iter()
                .map(|node_idx| {
                    ARROW_INTERSTATE_ROUTER
                        .as_ref()
                        .unwrap()
                        .get_node_by_id(*node_idx)
                        .unwrap()
                        .location
                })
                .collect::<Vec<Location>>();
            Ok(locations)
        },
    }
}

/// Initializes the router for the given aircraft
pub fn init_router(req: Aircraft) -> &'static str {
    if unsafe { NODES.is_none() } {
        return "Nodes not initialized. Try to get some nodes first.";
    }

    match req {
        Aircraft::ArrowXl => unsafe {
            if ARROW_XL_ROUTER.is_some() {
                return "Router already initialized. Try to use the router instead of initializing it.";
            }
            ARROW_XL_ROUTER = Some(Router::new(
                NODES.as_ref().unwrap(),
                ARROW_XL_CONSTRAINT,
                |from, to| haversine::distance(&from.as_node().location, &to.as_node().location),
                |from, to| haversine::distance(&from.as_node().location, &to.as_node().location),
            ));
            "Arrow XL router initialized."
        },
        Aircraft::ArrowCargo => unsafe {
            if ARROW_CARGO_ROUTER.is_some() {
                return "Router already initialized. Try to use the router instead of initializing it.";
            }
            ARROW_CARGO_ROUTER = Some(Router::new(
                NODES.as_ref().unwrap(),
                ARROW_CARGO_CONSTRAINT,
                |from, to| haversine::distance(&from.as_node().location, &to.as_node().location),
                |from, to| haversine::distance(&from.as_node().location, &to.as_node().location),
            ));
            "Arrow Cargo router initialized."
        },
        Aircraft::ArrowInterstate => unsafe {
            if ARROW_INTERSTATE_ROUTER.is_some() {
                return "Router already initialized. Try to use the router instead of initializing it.";
            }
            ARROW_INTERSTATE_ROUTER = Some(Router::new(
                NODES.as_ref().unwrap(),
                ARROW_INTERSTATE_CONSTRAINT,
                |from, to| haversine::distance(&from.as_node().location, &to.as_node().location),
                |from, to| haversine::distance(&from.as_node().location, &to.as_node().location),
            ));
            "Arrow Interstate router initialized."
        },
    }
}

/*
/// See if the router of the given aircraft has been initialized
pub async fn is_router_initialized(req: Json<Aircraft>) -> HttpResponse {
    unsafe {
        match req.into_inner() {
            Aircraft::ArrowXl => {
                if ARROW_XL_ROUTER.is_some() {
                    return HttpResponse::Ok().json(true);
                } else {
                    return HttpResponse::Ok().json(false);
                }
            }
            Aircraft::ArrowCargo => {
                if ARROW_CARGO_ROUTER.is_some() {
                    return HttpResponse::Ok().json(true);
                } else {
                    return HttpResponse::Ok().json(false);
                }
            }
            Aircraft::ArrowInterstate => {
                if ARROW_INTERSTATE_ROUTER.is_some() {
                    return HttpResponse::Ok().json(true);
                } else {
                    return HttpResponse::Ok().json(false);
                }
            }
        }
    }
}



/// Gets all edges in an aircraft's graph
pub async fn get_edges(req: Json<Aircraft>) -> HttpResponse {
    match req.into_inner() {
        Aircraft::ArrowXl => unsafe {
            if ARROW_XL_ROUTER.is_none() {
                return HttpResponse::InternalServerError()
                    .body("Arrow XL router not initialized. Try to initialize it first.");
            }
            return HttpResponse::Ok().json(ARROW_XL_ROUTER.as_ref().unwrap().get_edges());
        },
        Aircraft::ArrowCargo => unsafe {
            if ARROW_CARGO_ROUTER.is_none() {
                return HttpResponse::InternalServerError()
                    .body("Arrow Cargo router not initialized. Try to initialize it first.");
            }
            return HttpResponse::Ok().json(ARROW_CARGO_ROUTER.as_ref().unwrap().get_edges());
        },
        Aircraft::ArrowInterstate => unsafe {
            if ARROW_INTERSTATE_ROUTER.is_none() {
                return HttpResponse::InternalServerError()
                    .body("Arrow Interstate router not initialized. Try to initialize it first.");
            }
            return HttpResponse::Ok().json(ARROW_INTERSTATE_ROUTER.as_ref().unwrap().get_edges());
        },
    }
}

*/
