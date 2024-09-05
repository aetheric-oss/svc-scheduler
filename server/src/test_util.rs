/// test utilities. Provides functions to inject mock data.
use crate::grpc::client::get_clients;
use lib_common::log_macros;
use lib_common::time::{DateTime, Utc};
use lib_common::uuid::Uuid;
use svc_storage_client_grpc::prelude::*;
use tokio::sync::OnceCell;

log_macros!("ut", "test");

static INIT_MOCK_DATA: OnceCell<bool> = tokio::sync::OnceCell::const_new();
async fn init_mock_data() -> bool {
    let clients = get_clients().await;

    let vertiports = generate_vertiports(&clients.storage.vertiport).await;
    ut_debug!("Generated vertiports: {:#?}", vertiports);
    let vertipads = generate_vertipads(&clients.storage.vertipad, &vertiports).await;
    ut_debug!("Generated vertipads: {:#?}", vertipads);
    let vehicles = generate_vehicles(&clients.storage.vehicle, &vertiports).await;
    ut_debug!("Generated vehicles: {:#?}", vehicles);
    let flight_plans =
        generate_flight_plans(&clients.storage.flight_plan, &vertipads, &vehicles).await;
    ut_debug!("Generated flight_plans: {:#?}", flight_plans);
    let itinerary = generate_itinerary(
        &clients.storage.itinerary,
        &clients.storage.itinerary_flight_plan_link,
        &flight_plans,
    )
    .await;
    ut_debug!("Generated itinerary: {:#?}", itinerary);
    true
}

pub async fn ensure_storage_mock_data() {
    INIT_MOCK_DATA.get_or_init(init_mock_data).await;
}

pub async fn get_vertiports_from_storage() -> Result<Vec<vertiport::Object>, ()> {
    ensure_storage_mock_data().await;

    get_clients()
        .await
        .storage
        .vertiport
        .search(AdvancedSearchFilter {
            filters: vec![],
            page_number: 0,
            results_per_page: 100,
            order_by: vec![],
        })
        .await
        .map(|response| response.into_inner().list)
        .map_err(|e| {
            ut_error!("Could not find vertiports in MOCK service: {}", e);
        })
}

/// generate mock vertipads for the given vertiports
async fn generate_vertipads(
    client: &VertipadClient,
    vertiports: &Vec<vertiport::Object>,
) -> Vec<vertipad::Object> {
    let mut vertipads: Vec<vertipad::Object> = vec![];
    let sample_cal =
        "DTSTART:20221020T180000Z;DURATION:PT24H\nRRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR,SA,SU";

    for vertiport in vertiports {
        let mut vertipad = vertipad::mock::get_data_obj();
        vertipad.name = format!("Mock vertipad for vertiport {}", vertiport.id);
        vertipad.schedule = Some(String::from(sample_cal));
        vertipad.vertiport_id = vertiport.id.clone();

        let result: vertipad::Object = client
            .insert(vertipad.clone())
            .await
            .unwrap()
            .into_inner()
            .object
            .unwrap();
        vertipads.push(result);
    }

    // Insert a second vertipad for vertiport 4
    let vertiport = &vertiports[3];
    let mut vertipad = vertipad::mock::get_data_obj();
    vertipad.name = format!("Mock vertipad {}", vertiport.id);
    vertipad.schedule = Some(String::from(sample_cal));
    vertipad.vertiport_id = vertiport.id.clone();

    let result: vertipad::Object = client
        .insert(vertipad.clone())
        .await
        .unwrap()
        .into_inner()
        .object
        .unwrap();
    vertipads.push(result);

    vertipads
}

/// generate mock vertiports
async fn generate_vertiports(client: &VertiportClient) -> Vec<vertiport::Object> {
    let mut vertiports: Vec<vertiport::Object> = vec![];
    let sample_cal =
        "DTSTART:20221020T180000Z;DURATION:PT24H\nRRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR,SA,SU";

    let geo_locations = vec![
        GeoPolygonZ {
            rings: vec![GeoLineStringZ {
                points: vec![GeoPointZ {
                    y: 37.7931,
                    x: -122.46283,
                    z: 0.0,
                }],
            }],
        },
        GeoPolygonZ {
            rings: vec![GeoLineStringZ {
                points: vec![GeoPointZ {
                    y: 37.70278,
                    x: -122.42883,
                    z: 0.0,
                }],
            }],
        },
        GeoPolygonZ {
            rings: vec![GeoLineStringZ {
                points: vec![GeoPointZ {
                    y: 37.73278,
                    x: -122.45883,
                    z: 0.0,
                }],
            }],
        },
        GeoPolygonZ {
            rings: vec![GeoLineStringZ {
                points: vec![GeoPointZ {
                    y: 37.93278,
                    x: -122.25883,
                    z: 0.0,
                }],
            }],
        },
    ];
    for index in 0..geo_locations.len() {
        let mut vertiport = vertiport::mock::get_data_obj();
        vertiport.name = format!("Mock vertiport {}", index + 1);
        vertiport.geo_location = Some(geo_locations[index].clone());
        vertiport.schedule = Some(String::from(sample_cal));

        let result: vertiport::Object = client
            .insert(vertiport.clone())
            .await
            .unwrap()
            .into_inner()
            .object
            .unwrap();
        vertiports.push(result);
    }

    vertiports
}

/// generate mock vehicles for each of the given vertiports
/// vertiports will be used to determine vehicle's hangar_id
async fn generate_vehicles(
    client: &VehicleClient,
    vertiports: &Vec<vertiport::Object>,
) -> Vec<vehicle::Object> {
    let mut vehicles: Vec<vehicle::Object> = vec![];
    let sample_cal =
        "DTSTART:20221020T180000Z;DURATION:PT24H\nRRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR,SA,SU";

    // Vehicle at vertiport 1
    let mut vehicle = vehicle::mock::get_data_obj();
    vehicle.description = Some(format!("Mock vehicle {}", vertiports[0].id));
    vehicle.hangar_id = Some(vertiports[0].id.clone());
    vehicle.schedule = Some(String::from(sample_cal));

    let result: vehicle::Object = client
        .insert(vehicle.clone())
        .await
        .unwrap()
        .into_inner()
        .object
        .unwrap();
    vehicles.push(result);

    // Vehicle at vertiport 2
    let mut vehicle = vehicle::mock::get_data_obj();
    vehicle.description = Some(format!("Mock vehicle {}", vertiports[1].id));
    vehicle.hangar_id = Some(vertiports[1].id.clone());
    vehicle.schedule = Some(String::from(sample_cal));

    let result: vehicle::Object = client
        .insert(vehicle.clone())
        .await
        .unwrap()
        .into_inner()
        .object
        .unwrap();
    vehicles.push(result);

    // Vehicle at vertiport 3
    let mut vehicle = vehicle::mock::get_data_obj();
    vehicle.description = Some(format!("Mock vehicle {}", vertiports[2].id));
    vehicle.hangar_id = Some(vertiports[2].id.clone());
    vehicle.schedule = Some(String::from(sample_cal));

    let result: vehicle::Object = client
        .insert(vehicle.clone())
        .await
        .unwrap()
        .into_inner()
        .object
        .unwrap();
    vehicles.push(result);

    vehicles
}

async fn generate_flight_plans(
    client: &FlightPlanClient,
    vertipads: &Vec<vertipad::Object>,
    vehicles: &Vec<vehicle::Object>,
) -> Vec<flight_plan::Object> {
    vec![
        // 2022-10-25 |14:15|14:20|14:25|14:30|14:35|14:40|14:45|14:50|14:55|15:00|15:05|15:10|15:15|15:20|15:25|15:30|15:40|15:45|
        // ------------------------------------------------------------------------------------------------------------------------
        //            |     loading and takeoff
        // vertipad 1 |         <---fp 1--->
        //            |                      landing and unloading           loading and takeoff
        // vertipad 2 |                           <---fp 1--->                  <---fp 2--->
        //            |                                                                            landing and unloading
        // vertipad 3 |                                                                                 <---fp 2--->
        // ------------------------------------------------------------------------------------------------------------------------
        create_flight_plan(
            client,
            &vehicles[0].id,
            &vertipads[0],
            &vertipads[1],
            "2022-10-25 14:20:00",
            "2022-10-25 14:45:00",
        )
        .await,
        create_flight_plan(
            client,
            &vehicles[1].id,
            &vertipads[1],
            &vertipads[2],
            "2022-10-25 15:00:00",
            "2022-10-25 15:30:00",
        )
        .await,
        // 2022-10-26 |13:25|13:30|13:35|13:40|13:45|13:50|13:55|14:00|14:05|14:10|14:15|14:20|14:25|14:30|14:35|14:40|14:45|14:50|14:55|15:00|
        // ------------------------------------------------------------------------------------------------------------------------------------
        //            |                                                               landing and unloading    landing and unloading
        // vertipad 1 |                                                                     <-fp3-vh1-> <---free--> <-fp5-vh3->
        //            |                                         loading and takeoff                            landing and unloading
        // vertipad 2 |                                             <-fp3-vh1->                                     <-fp4-vh2->
        //            |     loading and takeoff
        // vertipad 3 |         <-fp4-vh2->
        // vertipad 3 |         <-fp5-vh3->  (double booked!?!?)
        // ------------------------------------------------------------------------------------------------------------------------------------
        create_flight_plan(
            client,
            &vehicles[0].id,
            &vertipads[1],
            &vertipads[0],
            "2022-10-26 14:00:00",
            "2022-10-26 14:30:00",
        )
        .await,
        create_flight_plan(
            client,
            &vehicles[1].id,
            &vertipads[2],
            &vertipads[1],
            "2022-10-26 13:30:00",
            "2022-10-26 14:50:00",
        )
        .await,
        create_flight_plan(
            client,
            &vehicles[2].id,
            &vertipads[2],
            &vertipads[0],
            "2022-10-26 13:30:00",
            "2022-10-26 14:50:00",
        )
        .await,
        //            |                            12                             |                             13                             |
        // 2022-10-27 | 00 | 05 | 10 | 15 | 20 | 25 | 30 | 35 | 40 | 45 | 50 | 55 |  00 | 05 | 10 | 15 | 20 | 25 | 30 | 35 | 40 | 45 | 50 | 55 |
        // ------------------------------------------------------------------------------------------------------------------------------------
        //            |                                                               landing and unloading    landing and unloading
        // vertipad 1 |                                                                     <-fp3-vh1-> <---free--> <-fp5-vh3->
        //            |                                         loading and takeoff                            landing and unloading
        // vertipad 2 |                                             <-fp3-vh1->                                     <-fp4-vh2->
        //            |     loading and takeoff
        // vertipad 3 |         <-fp4-vh2->
        // vertipad 3 |         <-fp5-vh3->  (double booked!?!?)
        create_flight_plan(
            client,
            &vehicles[0].id,
            &vertipads[0],
            &vertipads[1],
            "2022-10-27 12:00:00",
            "2022-10-27 13:00:00",
        )
        .await,
        create_flight_plan(
            client,
            &vehicles[1].id,
            &vertipads[1],
            &vertipads[2],
            "2022-10-27 12:00:00",
            "2022-10-27 13:00:00",
        )
        .await,
        create_flight_plan(
            client,
            &vehicles[2].id,
            &vertipads[2],
            &vertipads[0],
            "2022-10-27 12:00:00",
            "2022-10-27 12:20:00",
        )
        .await,
        create_flight_plan(
            client,
            &vehicles[2].id,
            &vertipads[0],
            &vertipads[3],
            "2022-10-27 14:00:00",
            "2022-10-27 15:00:00",
        )
        .await,
        create_flight_plan(
            client,
            &vehicles[1].id,
            &vertipads[1],
            &vertipads[3],
            "2022-10-27 15:00:00",
            "2022-10-27 15:50:00",
        )
        .await,
    ]
}

async fn create_flight_plan(
    client: &FlightPlanClient,
    vehicle_id: &str,
    origin_vertipad: &vertipad::Object,
    target_vertipad: &vertipad::Object,
    origin_time_str: &str,
    target_time_str: &str,
) -> flight_plan::Object {
    let mut flight_plan = flight_plan::mock::get_data_obj();
    flight_plan.vehicle_id = String::from(vehicle_id);
    flight_plan.origin_vertiport_id = Some(origin_vertipad.data.clone().unwrap().vertiport_id);
    flight_plan.target_vertiport_id = Some(target_vertipad.data.clone().unwrap().vertiport_id);
    flight_plan.origin_vertipad_id = origin_vertipad.id.clone();
    flight_plan.target_vertipad_id = target_vertipad.id.clone();
    flight_plan.origin_timeslot_start = Some(
        DateTime::parse_from_str(
            &(origin_time_str.to_owned() + " +0000"),
            "%Y-%m-%d %H:%M:%S %z",
        )
        .unwrap()
        .with_timezone(&Utc)
        .into(),
    );
    flight_plan.origin_timeslot_end = Some(
        DateTime::parse_from_str(
            &(origin_time_str.to_owned() + " +0000"),
            "%Y-%m-%d %H:%M:%S %z",
        )
        .unwrap()
        .with_timezone(&Utc)
        .into(),
    );

    flight_plan.target_timeslot_start = Some(
        DateTime::parse_from_str(
            &(target_time_str.to_owned() + " +0000"),
            "%Y-%m-%d %H:%M:%S %z",
        )
        .unwrap()
        .with_timezone(&Utc)
        .into(),
    );
    flight_plan.target_timeslot_end = Some(
        DateTime::parse_from_str(
            &(target_time_str.to_owned() + " +0000"),
            "%Y-%m-%d %H:%M:%S %z",
        )
        .unwrap()
        .with_timezone(&Utc)
        .into(),
    );

    flight_plan.flight_status = flight_plan::FlightStatus::Ready as i32;
    flight_plan.session_id = format!("AETH{}", rand::random::<u16>());

    client
        .insert(flight_plan)
        .await
        .unwrap()
        .into_inner()
        .object
        .unwrap()
}

/// generate mock itinerary for the given flight_plans
async fn generate_itinerary(
    client: &ItineraryClient,
    link_client: &ItineraryFlightPlanLinkClient,
    flight_plans: &Vec<flight_plan::Object>,
) -> Vec<itinerary::Object> {
    let mut itineraries: Vec<itinerary::Object> = vec![];

    let itinerary = itinerary::Data {
        user_id: Uuid::new_v4().to_string(),
        status: itinerary::ItineraryStatus::Active as i32,
    };

    let result: itinerary::Object = client
        .insert(itinerary.clone())
        .await
        .unwrap()
        .into_inner()
        .object
        .unwrap();
    itineraries.push(result.clone());

    let _result = link_client
        .link(itinerary::ItineraryFlightPlans {
            id: result.id,
            other_id_list: Some(IdList {
                ids: vec![flight_plans[0].id.clone(), flight_plans[1].id.clone()],
            }),
        })
        .await;

    itineraries
}
