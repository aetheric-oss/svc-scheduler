![Arrow Banner](https://github.com/Arrow-air/tf-github/raw/main/src/templates/doc-banner-services.png)

# Software Design Document (SDD) - `svc-scheduler`

## :telescope: Overview

This document details the software implementation of `svc-scheduler` (scheduler module).

The scheduler module is responsible for calculating possible itineraries (including deadhead flights) for a journey between a departure and destination vertipad. It does so with the schedules of all resources (vertiports/pads, aircrafts, pilots) in mind to avoid double-booking.

Draft itineraries are held in memory temporarily and discarded if not confirmed in time. Confirmed flights are saved to storage and can be cancelled. Flight queries, confirmations, and cancellation requests are made by other microservices in the Arrow network (such as `svc-cargo`).

*Note: This module is intended to be used by other Arrow micro-services via gRPC.*

*This document is under development as Arrow operates on a pre-revenue and pre-commercial stage. Scheduler logics may evolve as per business needs, which may result in architectural/implementation changes to the scheduler module.*

### Metadata

| Attribute     | Description                                                       |
| ------------- |-------------------------------------------------------------------|
| Maintainer(s) | [Services Team](https://github.com/orgs/Arrow-air/teams/services) |
| Stuckee       | [Alex M. Smith](https://github.com/servicedog)                   |
| Status        | Development                                                       |

## :books: Related Documents

Document | Description
--- | ----
[High-Level Concept of Operations (CONOPS)](https://github.com/Arrow-air/se-services/blob/develop/docs/conops.md) | Overview of Arrow microservices.
[High-Level Interface Control Document (ICD)](https://github.com/Arrow-air/se-services/blob/develop/docs/icd.md)  | Interfaces and frameworks common to all Arrow microservices.
[Requirements - `svc-scheduler`](https://nocodb.arrowair.com/dashboard/#/nc/view/bdffd78a-75bf-40b0-a45d-948cbee2241c) | Requirements and user stories for this microservice.
[Concept of Operations - `svc-scheduler`](./conops.md) | Defines the motivation and duties of this microservice.
[Interface Control Document - `svc-scheduler`](./icd.md)| Defines the inputs and outputs of this microservice.
[Routing Scenarios](https://docs.google.com/presentation/d/1Nt91KVIczhxngurfyeIJtG8J0m_38jGU1Cnqm1_BfPc/edit#slide=id.g1454d6dfbcf_0_731) | Graphical representation of various routing scenarios

## :dna: Module Attributes

| Attribute       | Applies | Explanation                                                                              |
|-----------------|---------|------------------------------------------------------------------------------------------|
| Safety Critical | No      | Scheduler is business critical but has no direct impact to the operational safety.       |
| Realtime        | No      | Scheduler is only used to fetch viable flights, and will not be used during the flights. |

## :gear: Logic

### Initialization

This module does not require user-side initialization.

The `main` function in [`/server/src/main.rs`](../server/src/main.rs) will simply spin up the server at the provided port.

### Environment Variables
The only environment variables are the port numbers used to spin up the server.

For the scheduler server, `DOCKER_PORT_GRPC` is the port number where the server lives. If not provided, `50051` will be used as a fallback port.

For the client, `HOST_PORT_GRPC` is needed to connect to the scheduler server. This env var should be the server's port. If not provided, `50051` will be used as a fallback port. In most cases, one may assume `HOST_PORT_GRPC` to have the same value as `DOCKER_PORT_GRPC`.

### Control Loop

Does not apply.

### Cleanup

Does not apply.

## :speech_balloon: gRPC Handlers

### `query_flight` 
- Takes requested departure and arrival vertiport ids and departure/arrival time window and returns next available flight(s).

```mermaid
sequenceDiagram
    participant client as gRPC Client

    client->>+scheduler: query_flight(...)<br>(Time Window,<br>Vertiports)
    alt for_each (target vertiports, vertipads, all aircraft, existing flight plans)
        scheduler->>storage: search(...)
        storage->>scheduler: Error
        scheduler->>client: GrpcError::Internal
        Note over client: end
    end
    
    Note over scheduler: Build vertipads' availabilities<br>given existing flight plans<br>and the vertipad's operating hours.
    
    alt for_each (possible departure timeslot, possible arrival timeslot)
        Note over scheduler: Calculate flight duration <br>between the two vertiports at<br>this time (considering temporary<br>no-fly zones and waypoints).
        Note over scheduler: If an aircraft can depart, travel,<br>and land within available<br>timeslots, append to results.
    end

    alt no timeslot pairings
        scheduler->>client: GrpcError::NotFound
        Note over client: end
    end

    Note over scheduler: Build aircraft availabilities<br>given existing flight plans.

    alt for_each aircraft and timeslot_pair
        Note over scheduler: If an aircraft is available<br>from the departure timeslot until<br>the arrival timeslot, append<br>the combination to results.
    end

    alt for_each (timeslot pair, aircraft availability)
        Note over scheduler: Calculate the duration<br> of the deadhead flight(s)<br>to the departure vertiport<br>and from the destination vertiport<br>to its next obligation.
        Note over scheduler: If the aircraft availability<br>can't fit either deadheads,<br> discard.
        Note over scheduler: Otherwise, append flight itinerary<br>to results and consider no other<br>itineraries with this specific<br>aircraft (max 1 result per aircraft).
    end

    alt no itineraries
        scheduler->>client: GrpcError::NotFound
        Note over client: end
    end

    alt for_each itinerary
        Note over scheduler: Create a draft itinerary in memory<br>with a unique UUID. Create draft<br>flight plans for each leg of the itinerary.
    end

    scheduler->>client: QueryFlightResponse<br>List of Itineraries with IDs

```

### `confirm_flight` 
- Takes id of the draft flight plan returned from the query_flight and confirms the flight.

```mermaid
sequenceDiagram
    grpc_client->>+scheduler: query_flight(QueryFlightRequest)
    scheduler->>scheduler: find draft flight plan in memory
    alt flight plan found
        scheduler->>+storage: insert_flight_plan(draft)
        storage->>-scheduler: <flight_plan> with permanent id
        scheduler->>scheduler: remove draft flight plan from memory
        scheduler->>-grpc_client: return <ConfirmFlightResponse> with permanent id
    else flight not found
        scheduler-->>grpc_client: return Error
    end
```


### `cancel_flight`
- Takes id of the flight plan (either draft or confirmed) and cancels the flight.
```mermaid
sequenceDiagram
    grpc_client->>+scheduler: query_flight(QueryFlightRequest)
    scheduler->>scheduler: find draft flight plan in memory
    alt flight plan found in memory
        scheduler->>scheduler: remove draft flight plan from memory
        scheduler->>grpc_client: return CancelFlightResponse
    else flight not found in memory 
        scheduler->>+storage: flight_plan_by_id(id)
        storage-->>-scheduler: <flight_plan> or None
        alt flight plan found in storage
            scheduler->>+storage: update_flight_plan(draft with status Cancelled)
            storage-->>-scheduler: Ok or Error
            scheduler->>grpc_client: return CancelFlightResponse
        else flight plan not found in storage
            scheduler-->>grpc_client: return Error
        end
    end
```

## Tests

### Unit Tests
