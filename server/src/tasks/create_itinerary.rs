use crate::grpc::client::{get_clients, GrpcClients};
use crate::router::flight_plan::{get_sorted_flight_plans, FlightPlanSchedule};
use crate::router::schedule::Timeslot;
use crate::router::vehicle::{get_aircraft, get_aircraft_availabilities};
use crate::router::vertiport::{get_timeslot_pairs, TimeslotPair};
use crate::tasks::{Task, TaskAction, TaskBody, TaskError};
use num_traits::FromPrimitive;
use std::collections::HashSet;
use svc_gis_client_grpc::client::UpdateFlightPathRequest;
use svc_gis_client_grpc::prelude::types::AircraftType;
use svc_gis_client_grpc::prelude::GisServiceClient;
use svc_storage_client_grpc::link_service::Client as LinkClient;
use svc_storage_client_grpc::prelude::flight_plan;
use svc_storage_client_grpc::prelude::{itinerary, IdList};
use svc_storage_client_grpc::simple_service::Client as SimpleClient;

/// Register flight plans with svc-storage
async fn register_flight_plans(
    user_id: &uuid::Uuid,
    flight_plans: &[TimeslotPair],
    aircraft_id: &str,
    clients: &GrpcClients,
) -> Result<(), TaskError> {
    //
    // TODO(R5): Do this in a transaction if possible, so that flight plans
    //  are rolled back if any part of the itinerary fails to be created.

    //
    // 1) Add flight plans to `flight_plan` DB table
    //
    let mut flight_plan_ids = vec![];
    for flight_plan in flight_plans.iter() {
        let mut tmp: flight_plan::Data = flight_plan.clone().into();
        tmp.vehicle_id = aircraft_id.to_string();

        let Ok(result) = clients.storage.flight_plan.insert(tmp).await else {
            tasks_error!("(register_flight_plans) Couldn't insert flight plan into storage.");
            return Err(TaskError::Internal);
        };

        let flight_id = match result.into_inner().object {
            Some(object) => object.id,
            None => {
                tasks_error!("(register_flight_plans) Couldn't insert flight plan into storage.");
                return Err(TaskError::Internal);
            }
        };

        let request = UpdateFlightPathRequest {
            flight_identifier: Some(flight_id.clone()),
            aircraft_identifier: Some(aircraft_id.to_string()),
            simulated: false,
            path: flight_plan.path.clone(),
            aircraft_type: AircraftType::Rotorcraft as i32, // TODO(R5): Get from storage
            timestamp_start: Some(flight_plan.origin_timeslot.time_end.into()),
            timestamp_end: Some(flight_plan.target_timeslot.time_start.into()),
        };

        clients.gis.update_flight_path(request).await.map_err(|e| {
            tasks_error!(
                "(register_flight_plans) Couldn't update flight path in GIS: {}",
                e
            );

            // TODO(R5): Rollback the changes in storage
            TaskError::Internal
        })?;

        flight_plan_ids.push(flight_id);
    }

    //
    // 2) Add itinerary to `itinerary` DB table
    //
    let data = itinerary::Data {
        user_id: user_id.to_string(),
        status: itinerary::ItineraryStatus::Active as i32,
    };

    let Ok(result) = clients.storage.itinerary.insert(data).await else {
        tasks_error!("(register_flight_plans) Couldn't insert itinerary into storage.");
        return Err(TaskError::Internal);
    };

    let itinerary_id = match result.into_inner().object {
        Some(object) => object.id,
        None => {
            tasks_error!("(register_flight_plans) Couldn't insert itinerary into storage.");
            return Err(TaskError::Internal);
        }
    };

    //
    // 3) Link flight plans to itinerary in `itinerary_flight_plan`
    //
    if let Err(e) = clients
        .storage
        .itinerary_flight_plan_link
        .link(itinerary::ItineraryFlightPlans {
            id: itinerary_id.clone(),
            other_id_list: Some(IdList {
                ids: flight_plan_ids,
            }),
        })
        .await
    {
        tasks_error!(
            "(register_flight_plans) Couldn't link flight plans to itinerary in storage: {}",
            e
        );
        return Err(TaskError::Internal);
    }

    Ok(())
}

/// Creates an itinerary given a list of flight plans, if valid
pub async fn create_itinerary(task: &mut Task) -> Result<(), TaskError> {
    let Some(TaskAction::CreateItinerary) = FromPrimitive::from_i32(task.metadata.action) else {
        tasks_error!(
            "(create_itinerary) Invalid task action: {}",
            task.metadata.action
        );
        return Err(TaskError::InvalidMetadata);
    };

    let user_id = match uuid::Uuid::parse_str(&task.metadata.user_id.clone()) {
        Ok(user_id) => user_id,
        Err(e) => {
            tasks_error!("(create_itinerary) Invalid user_id: {}", e);
            return Err(TaskError::InvalidUserId);
        }
    };

    let TaskBody::CreateItinerary(ref proposed_flight_plans) = task.body else {
        tasks_error!("(create_itinerary) Invalid task body: {:?}", task.body);
        return Err(TaskError::InvalidData);
    };

    // For retrieving asset information in one go
    let mut vertipad_ids = HashSet::new();
    let mut aircraft_id = String::new();
    if let Err(e) = crate::router::itinerary::validate_itinerary(
        proposed_flight_plans,
        &mut vertipad_ids,
        &mut aircraft_id,
    ) {
        let error_msg = "Invalid itinerary provided";
        tasks_error!("(create_itinerary) {error_msg}: {e}");
        return Err(TaskError::InvalidData);
    }

    if aircraft_id.is_empty() {
        tasks_error!("(create_itinerary) No aircraft provided.");
        return Err(TaskError::InvalidData);
    };

    let vertipad_ids = vertipad_ids.into_iter().collect::<Vec<String>>();

    //
    // Get total block of time needed by the aircraft
    //
    let Some(itinerary_start) = proposed_flight_plans.first() else {
        tasks_error!("(create_itinerary) No flight plans provided.");
        return Err(TaskError::InvalidData);
    };

    let Some(itinerary_end) = proposed_flight_plans.last() else {
        tasks_error!("(create_itinerary) No flight plans provided.");
        return Err(TaskError::InvalidData);
    };

    let aircraft_time_window = Timeslot {
        time_start: itinerary_start.origin_timeslot_start,
        time_end: itinerary_end.target_timeslot_end,
    };

    //
    // Get all aircraft schedules for the time window
    //
    let clients = get_clients().await;

    // Get all flight plans from this time to latest departure time (including partially fitting flight plans)
    // - this assumes that all landed flights have updated vehicle.last_vertiport_id (otherwise we would need to look in to the past)
    let existing_flight_plans: Vec<FlightPlanSchedule> =
        match get_sorted_flight_plans(clients, &aircraft_time_window.time_end).await {
            Ok(plans) => plans,
            Err(e) => {
                let error_str = "Could not get existing flight plans.";
                tasks_error!("(create_itinerary) {} {}", error_str, e);
                return Err(TaskError::Internal);
            }
        };

    // TODO(R5): For R4 we'll manually filter out the plans we don't care about
    //  in R5 if there's a more complicated way to form (A & B) || (C & D) type queries
    //  to storage we'll replace it.
    let existing_flight_plans = existing_flight_plans
        .into_iter()
        .filter(|plan| {
            // Filter out plans that are not in the vertipad list
            vertipad_ids.contains(&plan.origin_vertiport_id)
                || vertipad_ids.contains(&plan.target_vertiport_id)
                || plan.vehicle_id == aircraft_id
        })
        .collect::<Vec<FlightPlanSchedule>>();

    //
    // Get all aircraft availabilities
    //
    let Ok(aircraft) = get_aircraft(clients, Some(aircraft_id.clone())).await else {
        tasks_error!("(create_itinerary) Could not find aircraft.");
        return Err(TaskError::Internal);
    };

    //
    // Get the availability that contains at minimum the requested flight
    // The supplied itinerary (from query_itinerary) should also include the deadhead flights
    let mut aircraft_gaps =
        get_aircraft_availabilities(&existing_flight_plans, &aircraft, &aircraft_time_window)
            .map_err(|e| {
                tasks_error!("(create_itinerary) {}", e);
                TaskError::Internal
            })?;

    let Some(aircraft_gaps) = aircraft_gaps.remove(&aircraft_id) else {
        tasks_error!("(create_itinerary) Aircraft not available for the itinerary.");
        return Err(TaskError::ScheduleConflict);
    };

    if !aircraft_gaps.into_iter().any(|gap| {
        gap.vertiport_id == itinerary_start.origin_vertiport_id
            && gap.vertiport_id == itinerary_end.target_vertiport_id
            && gap.timeslot.time_start <= aircraft_time_window.time_start
            && gap.timeslot.time_end >= aircraft_time_window.time_end
    }) {
        tasks_error!("(create_itinerary) No available aircraft.");
        return Err(TaskError::ScheduleConflict);
    };

    // Get available timeslots for departure vertiport that are large enough to
    //  fit the required loading and takeoff time.
    //
    let mut pairs = vec![];
    for flight_plan in proposed_flight_plans {
        let loading_time = flight_plan.origin_timeslot_end - flight_plan.origin_timeslot_start;
        let unloading_time = flight_plan.target_timeslot_end - flight_plan.target_timeslot_start;
        let timeslot = Timeslot {
            time_start: flight_plan.origin_timeslot_start,
            time_end: flight_plan.target_timeslot_end,
        };

        let pair = get_timeslot_pairs(
            &flight_plan.origin_vertiport_id,
            Some(&flight_plan.origin_vertipad_id),
            &flight_plan.target_vertiport_id,
            Some(&flight_plan.target_vertipad_id),
            &loading_time,
            &unloading_time,
            &timeslot,
            &existing_flight_plans,
            clients,
        )
        .await
        .map_err(|e| {
            tasks_error!("(create_itinerary) {}", e);
            TaskError::ScheduleConflict
        })?
        .first()
        .ok_or_else(|| {
            tasks_info!("(create_itinerary) No routes available for the given time.");
            TaskError::ScheduleConflict
        })?
        .clone();

        pairs.push(pair);
    }

    // If we've reached this point, the itinerary is valid
    // Register it with svc-storage
    register_flight_plans(&user_id, &pairs, &aircraft_id, clients).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use cfg_if::cfg_if;

    cfg_if! {
        if #[cfg(feature = "stub_client")] {
            use crate::router::flight_plan::FlightPlanSchedule;
            use chrono::{Duration, Utc};
        }
    }

    use crate::tasks::{TaskAction, TaskBody, TaskMetadata};
    use uuid::Uuid;

    type TaskResult = Result<(), TaskError>;

    #[tokio::test]
    async fn ut_create_itinerary_invalid_task_body() -> TaskResult {
        let mut task = Task {
            metadata: TaskMetadata {
                action: TaskAction::CreateItinerary as i32,
                user_id: Uuid::new_v4().to_string(),
                ..Default::default()
            },
            body: TaskBody::CancelItinerary(Uuid::new_v4()),
        };

        let e = create_itinerary(&mut task).await.unwrap_err();
        assert_eq!(e, TaskError::InvalidData);

        Ok(())
    }

    #[tokio::test]
    async fn ut_create_itinerary_invalid_metadata() -> TaskResult {
        let mut task = Task {
            metadata: TaskMetadata {
                action: TaskAction::CancelItinerary as i32,
                user_id: Uuid::new_v4().to_string(),
                ..Default::default()
            },
            body: TaskBody::CreateItinerary(vec![]),
        };

        let e = create_itinerary(&mut task).await.unwrap_err();
        assert_eq!(e, TaskError::InvalidMetadata);

        let mut task = Task {
            metadata: TaskMetadata {
                action: TaskAction::CreateItinerary as i32,
                user_id: "invalid".to_string(),
                ..Default::default()
            },
            body: TaskBody::CreateItinerary(vec![]),
        };

        let e = create_itinerary(&mut task).await.unwrap_err();
        assert_eq!(e, TaskError::InvalidUserId);

        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "stub_client")]
    async fn ut_create_itinerary_schedule_conflict() -> TaskResult {
        let mut task = Task {
            metadata: TaskMetadata {
                action: TaskAction::CreateItinerary as i32,
                user_id: Uuid::new_v4().to_string(),
                ..Default::default()
            },
            body: TaskBody::CreateItinerary(vec![FlightPlanSchedule {
                origin_vertiport_id: Uuid::new_v4().to_string(),
                origin_vertipad_id: Uuid::new_v4().to_string(),
                origin_timeslot_start: Utc::now() + Duration::try_minutes(10).unwrap(),
                origin_timeslot_end: Utc::now() + Duration::try_minutes(11).unwrap(),
                target_vertiport_id: Uuid::new_v4().to_string(),
                target_vertipad_id: Uuid::new_v4().to_string(),
                target_timeslot_start: Utc::now() + Duration::try_minutes(30).unwrap(),
                target_timeslot_end: Utc::now() + Duration::try_minutes(31).unwrap(),
                vehicle_id: Uuid::new_v4().to_string(),
            }]),
        };

        let e = create_itinerary(&mut task).await.unwrap_err();
        assert_eq!(e, TaskError::ScheduleConflict);

        Ok(())
    }
}
