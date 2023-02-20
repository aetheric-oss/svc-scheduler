/// test utils for creating gRPC client stub
use crate::grpc_client_wrapper::{ComplianceClientWrapperTrait, StorageClientWrapperTrait};
use async_trait::async_trait;
use chrono::offset::Utc;
use chrono::TimeZone;
use once_cell::sync::OnceCell;
use prost_types::Timestamp;
use router::router_state::{init_router_from_vertiports, is_router_initialized};
use std::sync::Once;
use svc_compliance_client_grpc::client::{
    FlightPlanRequest, FlightPlanResponse, FlightReleaseRequest, FlightReleaseResponse,
};
use svc_storage_client_grpc::flight_plan::{
    Data as FlightPlanData, List as FlightPlans, Object as FlightPlan, Response as FPResponse,
    UpdateObject as UpdateFlightPlan,
};
use svc_storage_client_grpc::vehicle::{Data as VehicleData, List as Vehicles, Object as Vehicle};
use svc_storage_client_grpc::vertipad::{Data as VertipadData, Object as Vertipad};
use svc_storage_client_grpc::vertiport::{
    Data as VertiportData, List as Vertiports, Object as Vertiport,
};
use svc_storage_client_grpc::{AdvancedSearchFilter, Id};
use tonic::{Request, Response, Status};
use uuid::Uuid;

static INIT_LOGGER: Once = Once::new();
static INIT_ROUTER_STARTED: OnceCell<bool> = OnceCell::new();

pub async fn init_router(client_wrapper: &(dyn StorageClientWrapperTrait + Send + Sync)) {
    if is_router_initialized() {
        debug!("init_router Already initialized");
        return;
    }
    //this branch is needed to make sure that only one thread is initializing the router
    if INIT_ROUTER_STARTED.get().is_some() {
        debug!("init_router Some other thread is already initializing");
        tokio::time::sleep(core::time::Duration::from_millis(1500)).await;
        return;
    }
    INIT_ROUTER_STARTED.set(true).unwrap();
    debug!("init_router Starting to initialize router");
    let vertiports = client_wrapper
        .vertiports(Request::new(AdvancedSearchFilter {
            filters: vec![],
            order_by: vec![],
            page_number: 0,
            results_per_page: 0,
        }))
        .await
        .unwrap()
        .into_inner()
        .list;
    let _init_res = init_router_from_vertiports(&vertiports);
}

pub fn init_logger() {
    INIT_LOGGER.call_once(|| {
        let log_cfg: &str = "../log4rs.yaml";
        if let Err(e) = log4rs::init_file(log_cfg, Default::default()) {
            println!("(logger) could not parse {}. {}", log_cfg, e);
            panic!();
        }
    });
}

pub fn get_timestamp_from_utc_date(date: &str) -> Timestamp {
    let dt = Utc.datetime_from_str(date, "%Y-%m-%d %H:%M:%S").unwrap();
    Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    }
}

fn create_flight_plan(
    flight_plan_id: &str,
    vehicle_id: &str,
    departure_vertiport_id: &str,
    arrival_vertiport_id: &str,
    departure_time_str: &str,
    arrival_time_str: &str,
) -> FlightPlan {
    FlightPlan {
        id: flight_plan_id.parse().unwrap(),
        data: Some(FlightPlanData {
            pilot_id: "".to_string(),
            vehicle_id: vehicle_id.clone().parse().unwrap(),
            cargo_weight_grams: vec![],
            weather_conditions: None,
            departure_vertiport_id: Some(departure_vertiport_id.clone().parse().unwrap()),
            destination_vertiport_id: Some(arrival_vertiport_id.clone().parse().unwrap()),
            scheduled_departure: Some(get_timestamp_from_utc_date(departure_time_str)),
            scheduled_arrival: Some(get_timestamp_from_utc_date(arrival_time_str)),
            actual_departure: None,
            actual_arrival: None,
            flight_release_approval: None,
            flight_plan_submitted: None,
            approved_by: None,
            flight_status: 0,
            flight_priority: 0,
            departure_vertipad_id: departure_vertiport_id.to_owned() + &*"_n".to_string(),
            destination_vertipad_id: arrival_vertiport_id.to_owned() + &*"_n".to_string(),
            flight_distance_meters: 0,
        }),
    }
}

#[derive(Debug)]
pub struct ComplianceClientWrapperStub {}

#[async_trait]
impl ComplianceClientWrapperTrait for ComplianceClientWrapperStub {
    async fn submit_flight_plan(
        &self,
        _request: Request<FlightPlanRequest>,
    ) -> Result<Response<FlightPlanResponse>, Status> {
        Ok(Response::new(FlightPlanResponse {
            flight_plan_id: Uuid::new_v4().to_string(),
            submitted: true,
            result: None,
        }))
    }

    async fn request_flight_release(
        &self,
        _request: Request<FlightReleaseRequest>,
    ) -> Result<Response<FlightReleaseResponse>, Status> {
        Ok(Response::new(FlightReleaseResponse {
            flight_plan_id: Uuid::new_v4().to_string(),
            released: true,
            result: None,
        }))
    }
}

#[derive(Debug)]
pub struct StorageClientWrapperStub {
    vertiports: Vec<Vertiport>,
    vertipads: Vec<Vertipad>,
    flight_plans: Vec<FlightPlan>,
    vehicles: Vec<Vehicle>,
}

#[async_trait]
impl StorageClientWrapperTrait for StorageClientWrapperStub {
    async fn vertiports(
        &self,
        _request: Request<AdvancedSearchFilter>,
    ) -> Result<Response<Vertiports>, Status> {
        Ok(Response::new(Vertiports {
            list: self.vertiports.clone(),
        }))
    }

    async fn vertiport_by_id(&self, request: Request<Id>) -> Result<Response<Vertiport>, Status> {
        let id = request.into_inner().id;
        let vertiport = self
            .vertiports
            .iter()
            .find(|v| v.id == id)
            .ok_or_else(|| Status::not_found("Vertiport not found"))?
            .clone();
        Ok(Response::new(vertiport))
    }

    async fn flight_plan_by_id(
        &self,
        request: Request<Id>,
    ) -> Result<Response<FlightPlan>, Status> {
        let id = request.into_inner().id;
        let flight_plan = self
            .flight_plans
            .iter()
            .find(|v| v.id == id)
            .ok_or_else(|| Status::not_found("Flight plan not found"))?
            .clone();
        Ok(Response::new(flight_plan))
    }

    async fn flight_plans(
        &self,
        _request: Request<AdvancedSearchFilter>,
    ) -> Result<Response<FlightPlans>, Status> {
        Ok(Response::new(FlightPlans {
            list: self.flight_plans.clone(),
        }))
    }

    async fn insert_flight_plan(
        &self,
        request: Request<FlightPlanData>,
    ) -> Result<Response<FPResponse>, Status> {
        let flight_plan = FlightPlan {
            id: Uuid::new_v4().to_string(),
            data: Some(request.into_inner()),
        };
        //self.flight_plans.push(flight_plan.clone());
        Ok(Response::new(FPResponse {
            validation_result: None,
            object: Some(flight_plan),
        }))
    }

    async fn update_flight_plan(
        &self,
        request: Request<UpdateFlightPlan>,
    ) -> Result<Response<FPResponse>, Status> {
        let update_flight_plan = request.into_inner();
        let id = update_flight_plan.id;
        let mut flight_plan = self
            .flight_plans
            .iter()
            .find(|v| v.id == id)
            .ok_or_else(|| Status::not_found("Flight plan not found"))?
            .clone();
        flight_plan.data = update_flight_plan.data;
        Ok(Response::new(FPResponse {
            validation_result: None,
            object: Some(flight_plan),
        }))
    }

    async fn vehicles(
        &self,
        _request: Request<AdvancedSearchFilter>,
    ) -> Result<Response<Vehicles>, Status> {
        Ok(Response::new(Vehicles {
            list: self.vehicles.clone(),
        }))
    }
}

pub fn create_compliance_client_stub() -> ComplianceClientWrapperStub {
    ComplianceClientWrapperStub {}
}

pub fn create_storage_client_stub() -> StorageClientWrapperStub {
    let sample_cal =
        "DTSTART:20221020T180000Z;DURATION:PT1H\nRRULE:FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR";
    let vertiports = vec![
        Vertiport {
            id: "vertiport1".to_string(),
            data: Some(VertiportData {
                name: "".to_string(),
                description: "".to_string(),
                latitude: 37.79310,
                longitude: -122.46283,
                schedule: Some(sample_cal.to_string()),
            }),
        },
        Vertiport {
            id: "vertiport2".to_string(),
            data: Some(VertiportData {
                name: "".to_string(),
                description: "".to_string(),
                latitude: 37.70278,
                longitude: -122.42883,
                schedule: Some(sample_cal.to_string()),
            }),
        },
        Vertiport {
            id: "vertiport3".to_string(),
            data: Some(VertiportData {
                name: "".to_string(),
                description: "".to_string(),
                latitude: 37.73278,
                longitude: -122.45883,
                schedule: Some(sample_cal.to_string()),
            }),
        },
    ];
    let vertipads = vec![
        Vertipad {
            id: "vertipad11".to_string(),
            data: Some(VertipadData {
                vertiport_id: "vertiport1".to_string(),
                name: "".to_string(),
                latitude: 0.0,
                longitude: 0.0,
                enabled: false,
                occupied: false,
                schedule: Some(sample_cal.to_string()),
            }),
        },
        Vertipad {
            id: "vertipad12".to_string(),
            data: Some(VertipadData {
                vertiport_id: "vertiport1".to_string(),
                name: "".to_string(),
                latitude: 0.0,
                longitude: 0.0,
                enabled: false,
                occupied: false,
                schedule: Some(sample_cal.to_string()),
            }),
        },
        Vertipad {
            id: "vertipad21".to_string(),
            data: Some(VertipadData {
                vertiport_id: "vertiport2".to_string(),
                name: "".to_string(),
                latitude: 0.0,
                longitude: 0.0,
                enabled: false,
                occupied: false,
                schedule: Some(sample_cal.to_string()),
            }),
        },
        Vertipad {
            id: "vertipad31".to_string(),
            data: Some(VertipadData {
                vertiport_id: "vertiport3".to_string(),
                name: "".to_string(),
                latitude: 0.0,
                longitude: 0.0,
                enabled: false,
                occupied: false,
                schedule: Some(sample_cal.to_string()),
            }),
        },
    ];
    let flight_plans = vec![
        create_flight_plan(
            "flight_plan1",
            "vehicle1",
            "vertiport1",
            "vertiport2",
            "2022-10-25 14:20:00",
            "2022-10-25 14:45:00",
        ),
        create_flight_plan(
            "flight_plan2",
            "vehicle2",
            "vertiport2",
            "vertiport3",
            "2022-10-25 15:00:00",
            "2022-10-25 15:30:00",
        ),
        create_flight_plan(
            "flight_plan3",
            "vehicle1",
            "vertiport2",
            "vertiport1",
            "2022-10-26 14:00:00",
            "2022-10-26 14:30:00",
        ),
        create_flight_plan(
            "flight_plan4",
            "vehicle2",
            "vertiport3",
            "vertiport2",
            "2022-10-26 13:30:00",
            "2022-10-26 14:50:00",
        ),
        create_flight_plan(
            "flight_plan5",
            "vehicle3",
            "vertiport3",
            "vertiport1",
            "2022-10-26 13:30:00",
            "2022-10-26 14:50:00",
        ),
        create_flight_plan(
            "flight_plan6",
            "vehicle1",
            "vertiport1",
            "vertiport2",
            "2022-10-27 12:00:00",
            "2022-10-27 13:00:00",
        ),
        create_flight_plan(
            "flight_plan7",
            "vehicle2",
            "vertiport2",
            "vertiport3",
            "2022-10-27 12:00:00",
            "2022-10-27 13:00:00",
        ),
        create_flight_plan(
            "flight_plan8",
            "vehicle3",
            "vertiport3",
            "vertiport1",
            "2022-10-27 12:00:00",
            "2022-10-27 12:20:00",
        ),
    ];
    let vehicles = vec![
        Vehicle {
            id: "vehicle1".to_string(),
            data: Some(VehicleData {
                vehicle_model_id: "".to_string(),
                serial_number: "".to_string(),
                registration_number: "".to_string(),
                description: Some("".to_string()),
                asset_group_id: None,
                schedule: Some(sample_cal.to_string()),
                last_maintenance: None,
                next_maintenance: None,
                last_vertiport_id: Some("vertiport1".to_string()),
            }),
        },
        Vehicle {
            id: "vehicle2".to_string(),
            data: Some(VehicleData {
                vehicle_model_id: "".to_string(),
                serial_number: "".to_string(),
                registration_number: "".to_string(),
                description: Some("".to_string()),
                asset_group_id: None,
                schedule: Some(sample_cal.to_string()),
                last_maintenance: None,
                next_maintenance: None,
                last_vertiport_id: Some("vertiport2".to_string()),
            }),
        },
        Vehicle {
            id: "vehicle3".to_string(),
            data: Some(VehicleData {
                vehicle_model_id: "".to_string(),
                serial_number: "".to_string(),
                registration_number: "".to_string(),
                description: Some("".to_string()),
                asset_group_id: None,
                schedule: Some(sample_cal.to_string()),
                last_maintenance: None,
                next_maintenance: None,
                last_vertiport_id: Some("vertiport3".to_string()),
            }),
        },
    ];

    StorageClientWrapperStub {
        vertiports,
        vertipads,
        flight_plans,
        vehicles,
    }
}
