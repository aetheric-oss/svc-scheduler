use crate::grpc::client::{get_clients, GrpcClients};
use crate::router::flight_plan::{get_sorted_flight_plans, FlightPlanSchedule};
use crate::router::schedule::Timeslot;
use crate::router::vehicle::{get_aircraft, get_aircraft_availabilities};
use crate::tasks::{Task, TaskAction, TaskBody, TaskError};
use lib_common::uuid::Uuid;
use num_traits::FromPrimitive;
use std::collections::HashSet;
use svc_gis_client_grpc::client::{CheckIntersectionRequest, UpdateFlightPathRequest};
use svc_gis_client_grpc::prelude::types::AircraftType;
use svc_gis_client_grpc::prelude::GisServiceClient;
use svc_storage_client_grpc::link_service::Client as LinkClient;
use svc_storage_client_grpc::prelude::flight_plan;
use svc_storage_client_grpc::prelude::{itinerary, Id, IdList};
use svc_storage_client_grpc::simple_service::Client as SimpleClient;

const SESSION_ID_PREFIX: &str = "AETH";

/// Register flight plans with svc-storage and return the itinerary ID
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) needs running backend, integration tests
async fn register_flight_plans(
    clients: &GrpcClients,
    user_id: &Uuid,
    flight_plans: &[FlightPlanSchedule],
) -> Result<String, TaskError> {
    //
    // TODO(R5): Do this in a transaction if possible, so that flight plans
    //  are rolled back if any part of the itinerary fails to be created.

    //
    // 1) Add flight plans to `flight_plan` DB table
    //
    let mut flight_plan_ids = vec![];
    for flight_plan in flight_plans.iter() {
        // TODO(R5): This is a temporary solution to generate a session id
        //  should be replaced with a proper session id generator that won't
        //  conflict with an active or future ID already in storage
        let session_id = format!("{SESSION_ID_PREFIX}{}", rand::random::<u16>());
        let mut tmp: flight_plan::Data = flight_plan.clone().into();
        tmp.session_id = session_id.clone();
        tmp.pilot_id = Uuid::new_v4().to_string(); // TODO(R5): Pilots not currently supported
        let path = flight_plan.path.clone().ok_or_else(|| {
            tasks_error!("Flight plan has no path.");
            TaskError::Internal
        })?;

        let result = clients
            .storage
            .flight_plan
            .insert(tmp)
            .await
            .map_err(|e| {
                tasks_error!("Couldn't insert flight plan into storage: {}", e);
                TaskError::Internal
            })?
            .into_inner()
            .object
            .ok_or_else(|| {
                tasks_error!("Couldn't insert flight plan into storage.");
                TaskError::Internal
            })?;

        let flight_id = result.id.clone();
        let session_id = result
            .data
            .ok_or_else(|| {
                tasks_error!("Flight plan object had no data.");
                TaskError::Internal
            })?
            .session_id; // the short flight id (i.e. KLM 1234)

        let registration_id = clients
            .storage
            .vehicle
            .get_by_id(Id {
                id: flight_plan.vehicle_id.to_string(),
            })
            .await
            .map_err(|e| {
                tasks_error!("Couldn't get aircraft information from storage: {}", e);
                TaskError::Internal
            })?
            .into_inner()
            .data
            .ok_or_else(|| {
                tasks_error!("Vehicle object had no data.");
                TaskError::Internal
            })?
            .registration_number; // the tail number

        let request = UpdateFlightPathRequest {
            flight_identifier: Some(session_id.clone()),
            aircraft_identifier: Some(registration_id.to_string()),
            simulated: false,
            path,
            aircraft_type: AircraftType::Rotorcraft as i32, // TODO(R5): Get from storage
            timestamp_start: Some(flight_plan.origin_timeslot_end.into()),
            timestamp_end: Some(flight_plan.target_timeslot_start.into()),
        };

        clients.gis.update_flight_path(request).await.map_err(|e| {
            tasks_error!("Couldn't update flight path in GIS: {}", e);

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

    let itinerary_id = clients
        .storage
        .itinerary
        .insert(data)
        .await
        .map_err(|e| {
            tasks_error!("Couldn't insert itinerary into storage: {}", e);
            TaskError::Internal
        })?
        .into_inner()
        .object
        .ok_or_else(|| {
            tasks_error!("Couldn't insert itinerary into storage.");
            TaskError::Internal
        })?
        .id;

    //
    // 3) Link flight plans to itinerary in `itinerary_flight_plan`
    //
    let _ = clients
        .storage
        .itinerary_flight_plan_link
        .link(itinerary::ItineraryFlightPlans {
            id: itinerary_id.clone(),
            other_id_list: Some(IdList {
                ids: flight_plan_ids,
            }),
        })
        .await
        .map_err(|e| {
            tasks_error!("Couldn't link flight plans to itinerary in storage: {}", e);
            TaskError::Internal
        })?;

    tasks_info!("Registered itinerary: {}", itinerary_id);
    Ok(itinerary_id)
}

/// Creates an itinerary given a list of flight plans, if valid
#[cfg(not(tarpaulin_include))]
// no_coverage: (R5) needs running backend, integration tests
pub async fn create_itinerary(task: &mut Task) -> Result<(), TaskError> {
    let Some(TaskAction::CreateItinerary) = FromPrimitive::from_i32(task.metadata.action) else {
        tasks_error!("Invalid task action: {}", task.metadata.action);

        return Err(TaskError::Metadata);
    };

    let user_id = Uuid::parse_str(&task.metadata.user_id.clone()).map_err(|e| {
        tasks_error!("Invalid user_id: {}", e);
        TaskError::UserId
    })?;

    let TaskBody::CreateItinerary(ref proposed_flight_plans) = task.body else {
        tasks_error!("Invalid task body: {:?}", task.body);
        return Err(TaskError::Data);
    };

    // For retrieving asset information in one go
    let mut vertipad_ids = HashSet::new();
    let mut aircraft_id = String::new();

    // Validate the itinerary request
    crate::router::itinerary::validate_itinerary(
        proposed_flight_plans,
        &mut vertipad_ids,
        &mut aircraft_id,
    )
    .map_err(|e| {
        tasks_error!("Invalid itinerary provided: {}", e);
        TaskError::Data
    })?;

    //
    // Get total block of time needed by the aircraft
    //
    let itinerary_start = proposed_flight_plans.first().ok_or_else(|| {
        tasks_error!("No flight plans provided.");
        TaskError::Data
    })?;

    let itinerary_end = proposed_flight_plans.last().ok_or_else(|| {
        tasks_error!("No flight plans provided.");
        TaskError::Data
    })?;

    let aircraft_time_window = Timeslot::new(
        itinerary_start.origin_timeslot_start,
        itinerary_end.target_timeslot_end,
    )
    .map_err(|e| {
        tasks_error!("Invalid aircraft time window: {}", e);
        TaskError::Internal
    })?;

    //
    // Fast intersection check before collecting all sorts of data
    //
    let clients = get_clients().await;
    for flight_plan in proposed_flight_plans {
        let path = flight_plan.path.clone().ok_or_else(|| {
            tasks_error!("Flight plan has no path.");
            TaskError::Data
        })?;

        let request = CheckIntersectionRequest {
            path,
            time_start: Some(flight_plan.origin_timeslot_end.into()),
            time_end: Some(flight_plan.target_timeslot_start.into()),
            origin_identifier: flight_plan.origin_vertiport_id.clone(),
            target_identifier: flight_plan.target_vertiport_id.clone(),
        };

        clients
            .gis
            .check_intersection(request)
            .await
            .map_err(|e| {
                tasks_error!("couldn't check intersection {}", e);
                TaskError::Internal
            })?
            .into_inner()
            .intersects
            .then_some(())
            .ok_or_else(|| {
                tasks_error!("Flight plan intersects with another flight plan");
                TaskError::ScheduleConflict
            })?;
    }

    // Get all flight plans from this time to latest departure time (including partially fitting flight plans)
    // - this assumes that all landed flights have updated vehicle.last_vertiport_id (otherwise we would need to look in to the past)
    // TODO(R5): For R4 we'll manually filter out the plans we don't care about
    //  in R5 if there's a more complicated way to form (A & B) || (C & D) type queries
    //  to storage we'll replace it.
    // let vertipad_ids = vertipad_ids.into_iter().collect::<Vec<String>>();
    let existing_flight_plans: Vec<FlightPlanSchedule> = get_sorted_flight_plans(clients)
        .await
        .map_err(|e| {
            tasks_error!("Could not get existing flight plans: {}", e);
            TaskError::Internal
        })?
        .into_iter()
        .filter(|plan| {
            // Filter out plans that are not in the vertipad list
            vertipad_ids.contains(&plan.origin_vertipad_id)
                || vertipad_ids.contains(&plan.target_vertipad_id)
                || plan.vehicle_id == aircraft_id
        })
        .collect::<Vec<FlightPlanSchedule>>();

    //
    // Get all aircraft availabilities
    //
    let aircraft = get_aircraft(clients, Some(aircraft_id.clone()))
        .await
        .map_err(|e| {
            tasks_error!("{}", e);
            TaskError::Internal
        })?;

    //
    // Get the availability that contains at minimum the requested flight
    // The supplied itinerary (from query_itinerary) should also include the deadhead flights
    //
    let aircraft_gaps = get_aircraft_availabilities(
        &existing_flight_plans,
        &aircraft_time_window.time_start(),
        &aircraft,
        &aircraft_time_window,
    )
    .map_err(|e| {
        tasks_error!("{}", e);
        TaskError::Internal
    })?
    .remove(&aircraft_id)
    .ok_or_else(|| {
        tasks_error!("Aircraft not available for the itinerary.");
        TaskError::ScheduleConflict
    })?;

    if !aircraft_gaps.into_iter().any(|gap| {
        gap.vertiport_id == itinerary_start.origin_vertiport_id
            && gap.vertiport_id == itinerary_end.target_vertiport_id
            && gap.timeslot.time_start() <= aircraft_time_window.time_start()
            && gap.timeslot.time_end() >= aircraft_time_window.time_end()
    }) {
        tasks_error!("The requested aircraft is not available.");
        return Err(TaskError::ScheduleConflict);
    };

    //
    // Get timeslots for each vertipad mentioned in the plan
    //
    let timeslot = Timeslot::new(
        aircraft_time_window.time_start(),
        aircraft_time_window.time_end(),
    )
    .map_err(|e| {
        tasks_error!("invalid aircraft time window: {}", e);
        TaskError::Internal
    })?;

    for flight_plan in proposed_flight_plans {
        let origin_duration = flight_plan.origin_timeslot_end - flight_plan.origin_timeslot_start;
        let origin_timeslots = crate::router::vertiport::get_available_timeslots(
            &flight_plan.origin_vertiport_id,
            Some(&flight_plan.origin_vertipad_id),
            &existing_flight_plans,
            &timeslot,
            &origin_duration,
            clients,
        )
        .await
        .map_err(|e| {
            tasks_error!("{}", e);
            TaskError::Internal
        })?
        .remove(&flight_plan.origin_vertipad_id)
        .ok_or_else(|| {
            tasks_error!("No timeslots available for this vertipad.");
            TaskError::ScheduleConflict
        })?;

        if !origin_timeslots.into_iter().any(|gap| {
            gap.time_start() <= flight_plan.origin_timeslot_start
                && gap.time_end() >= flight_plan.origin_timeslot_end
        }) {
            tasks_error!("This requested timeslot is not available.");
            return Err(TaskError::ScheduleConflict);
        };

        let target_duration = flight_plan.target_timeslot_end - flight_plan.target_timeslot_start;
        let target_timeslots = crate::router::vertiport::get_available_timeslots(
            &flight_plan.target_vertiport_id,
            Some(&flight_plan.target_vertipad_id),
            &existing_flight_plans,
            &timeslot,
            &target_duration,
            clients,
        )
        .await
        .map_err(|e| {
            tasks_error!("{}", e);
            TaskError::Internal
        })?
        .remove(&flight_plan.target_vertipad_id)
        .ok_or_else(|| {
            tasks_error!("No timeslots available for this vertipad.");
            TaskError::ScheduleConflict
        })?;

        if !target_timeslots.into_iter().any(|gap| {
            gap.time_start() <= flight_plan.target_timeslot_start
                && gap.time_end() >= flight_plan.target_timeslot_end
        }) {
            tasks_error!("This requested timeslot is not available.");
            return Err(TaskError::ScheduleConflict);
        };
    }

    // If we've reached this point, the itinerary is valid
    // Register it with svc-storage
    let itinerary_id = register_flight_plans(clients, &user_id, proposed_flight_plans).await?;
    task.metadata.result = Some(itinerary_id);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cfg_if::cfg_if;

    cfg_if! {
        if #[cfg(feature = "stub_client")] {
            use crate::router::flight_plan::FlightPlanSchedule;
            use lib_common::time::{Duration, Utc};
        }
    }

    use crate::tasks::{TaskAction, TaskBody, TaskMetadata};

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
        assert_eq!(e, TaskError::Data);

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
        assert_eq!(e, TaskError::Metadata);

        let mut task = Task {
            metadata: TaskMetadata {
                action: TaskAction::CreateItinerary as i32,
                user_id: "invalid".to_string(),
                ..Default::default()
            },
            body: TaskBody::CreateItinerary(vec![]),
        };

        let e = create_itinerary(&mut task).await.unwrap_err();
        assert_eq!(e, TaskError::UserId);

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
                path: Some(vec![]),
            }]),
        };

        let e = create_itinerary(&mut task).await.unwrap_err();
        assert_eq!(e, TaskError::ScheduleConflict);

        Ok(())
    }
}
