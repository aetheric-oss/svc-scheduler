/// test utils for creating gRPC client stub
use crate::grpc_client_wrapper::{GRPCClients, StorageClientWrapperTrait};
use async_trait::async_trait;
use svc_storage_client_grpc::client::{
    FlightPlan, FlightPlanData, FlightPlans, Id, SearchFilter, UpdateFlightPlan, Vehicle,
    VehicleData, Vehicles, Vertipad, VertipadData, Vertiport, VertiportData, Vertiports,
};
use tonic::{Request, Response, Status};
use uuid::Uuid;

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
        request: Request<SearchFilter>,
    ) -> Result<Response<Vertiports>, Status> {
        Ok(Response::new(Vertiports {
            vertiports: self.vertiports.clone(),
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
        request: Request<SearchFilter>,
    ) -> Result<Response<FlightPlans>, Status> {
        Ok(Response::new(FlightPlans {
            flight_plans: self.flight_plans.clone(),
        }))
    }

    async fn insert_flight_plan(
        &self,
        request: Request<FlightPlanData>,
    ) -> Result<Response<FlightPlan>, Status> {
        let flight_plan = FlightPlan {
            id: Uuid::new_v4().to_string(),
            data: Some(request.into_inner()),
        };
        //self.flight_plans.push(flight_plan.clone());
        Ok(Response::new(flight_plan))
    }

    async fn update_flight_plan(
        &self,
        request: Request<UpdateFlightPlan>,
    ) -> Result<Response<FlightPlan>, Status> {
        let update_flight_plan = request.into_inner();
        let id = update_flight_plan.id;
        let mut flight_plan = self
            .flight_plans
            .iter()
            .find(|v| v.id == id)
            .ok_or_else(|| Status::not_found("Flight plan not found"))?
            .clone();
        flight_plan.data = update_flight_plan.data;
        Ok(Response::new(flight_plan))
    }

    async fn vehicles(&self, request: Request<SearchFilter>) -> Result<Response<Vehicles>, Status> {
        Ok(Response::new(Vehicles {
            vehicles: self.vehicles.clone(),
        }))
    }
}

pub fn create_storage_client_stub() -> StorageClientWrapperStub {
    let sample_cal =
        "DTSTART:20221020T180000Z;DURATION:PT1H\nRRULE:FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR";
    let vertiports = vec![
        Vertiport {
            id: "vertiport1".to_string(),
            data: Some(VertiportData {
                description: "".to_string(),
                latitude: 37.79310,
                longitude: -122.46283,
                schedule: Some(sample_cal.to_string()),
            }),
        },
        Vertiport {
            id: "vertiport2".to_string(),
            data: Some(VertiportData {
                description: "".to_string(),
                latitude: 37.70278,
                longitude: -122.42883,
                schedule: Some(sample_cal.to_string()),
            }),
        },
    ];
    let vertipads = vec![
        Vertipad {
            id: "vertipad1".to_string(),
            data: Some(VertipadData {
                vertiport_id: "vertiport1".to_string(),
                description: "".to_string(),
                latitude: 0.0,
                longitude: 0.0,
                enabled: false,
                occupied: false,
                schedule: Some(sample_cal.to_string()),
            }),
        },
        Vertipad {
            id: "vertipad2".to_string(),
            data: Some(VertipadData {
                vertiport_id: "vertiport1".to_string(),
                description: "".to_string(),
                latitude: 0.0,
                longitude: 0.0,
                enabled: false,
                occupied: false,
                schedule: Some(sample_cal.to_string()),
            }),
        },
    ];
    let flight_plans = vec![
        FlightPlan {
            id: "flight_plan1".to_string(),
            data: Some(FlightPlanData {
                pilot_id: "".to_string(),
                vehicle_id: "".to_string(),
                cargo_weight_g: vec![],
                flight_distance: 0,
                weather_conditions: "".to_string(),
                departure_vertiport_id: None,
                departure_vertipad_id: "".to_string(),
                destination_vertiport_id: None,
                destination_vertipad_id: "vertipad2".to_string(),
                scheduled_departure: None,
                scheduled_arrival: None,
                actual_departure: None,
                actual_arrival: None,
                flight_release_approval: None,
                flight_plan_submitted: None,
                approved_by: None,
                flight_status: 0,
                flight_priority: 0,
            }),
        },
        FlightPlan {
            id: "flight_plan2".to_string(),
            data: Some(FlightPlanData {
                pilot_id: "".to_string(),
                vehicle_id: "".to_string(),
                cargo_weight_g: vec![],
                flight_distance: 0,
                weather_conditions: "".to_string(),
                departure_vertiport_id: None,
                departure_vertipad_id: "".to_string(),
                destination_vertipad_id: "vertipad1".to_string(),
                scheduled_departure: None,
                scheduled_arrival: None,
                actual_departure: None,
                actual_arrival: None,
                flight_release_approval: None,
                flight_plan_submitted: None,
                approved_by: None,
                flight_status: 0,
                destination_vertiport_id: None,
                flight_priority: 0,
            }),
        },
    ];
    let vehicles = vec![
        Vehicle {
            id: "vehicle1".to_string(),
            data: Some(VehicleData {
                vehicle_type: 0,
                description: "".to_string(),
                schedule: Some(sample_cal.to_string()),
            }),
        },
        Vehicle {
            id: "vehicle2".to_string(),
            data: Some(VehicleData {
                vehicle_type: 0,
                description: "".to_string(),
                schedule: Some(sample_cal.to_string()),
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
