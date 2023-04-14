# `svc-scheduler`- Software Design Document (SDD)

<center>

<img src="https://github.com/Arrow-air/tf-github/raw/main/src/templates/doc-banner-services.png" style="height:250px" />

</center>

### Metadata

| Item | Description                                                       |
| --- |-------------------------------------------------------------------|
| Maintainer(s) | [Services Team](https://github.com/orgs/Arrow-air/teams/services) |
| Primary Contact | [romanmandryk](https://github.com/romanmandryk)                      |

## Overview

Attribute | Description
--- | ---
Status | :yellow_circle: Development

This document details the software implementation of `svc-scheduler` (scheduler module).

The scheduler module is responsible for calculating possible itineraries (including deadhead flights) for a journey between a departure and destination vertipad. It does so with the schedules of all resources (vertiports/pads, aircrafts, pilots) in mind to avoid double-booking.

Draft itineraries are held in memory temporarily and discarded if not confirmed in time. Confirmed flights are saved to storage and can be cancelled. Flight queries, confirmations, and cancellation requests are made by other microservices in the Arrow network (such as `svc-cargo`).

*Note: This module is intended to be used by other Arrow micro-services via gRPC.*

*This document is under development as Arrow operates on a pre-revenue and pre-commercial stage. Scheduler logics may evolve as per business needs, which may result in architectural/implementation changes to the scheduler module.*

## Related Documents

Document | Description
--- | ----
[High-Level Concept of Operations (CONOPS)](https://github.com/Arrow-air/se-services/blob/develop/docs/conops.md) | Overview of Arrow microservices.
[High-Level Interface Control Document (ICD)](https://github.com/Arrow-air/se-services/blob/develop/docs/icd.md)  | Interfaces and frameworks common to all Arrow microservices.
[Concept of Operations - `svc-scheduler`](./conops.md) | Concept of Operations for `svc-scheduler`.
[Interface Control Document - `svc-scheduler`](./icd.md)| Interface Control Document for `svc-scheduler`.
[Requirements - `svc-scheduler`](https://nocodb.arrowair.com/dashboard/#/nc/view/bdffd78a-75bf-40b0-a45d-948cbee2241c) | Requirements for this service.

## Location

Server-side service.

## Module Attributes

| Attribute       | Applies | Explanation                                                                              |
|-----------------|---------|------------------------------------------------------------------------------------------|
| Safety Critical | No      | Scheduler is business critical but has no direct impact to the operational safety.       |
| Realtime        | No      | Scheduler is only used to fetch viable flights, and will not be used during the flights. |


## Logic

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

## Interface Handlers

See [the ICD](./icd.md) for this microservice.

### `query_flight` 
- Takes requested departure and arrival vertiport ids and departure/arrival time window and returns next available flight(s).

```mermaid
sequenceDiagram
    grpc_client->>+scheduler: query_flight(QueryFlightRequest)
    scheduler->>+storage: get depart and arrive vertiports
    storage->>-scheduler: <vertiports>
    scheduler->>+storage: get flights scheduled from depart and to arrive vertiports
    storage->>-scheduler: <flight_plans>
    scheduler->>scheduler: Check there are now flights scheduled for the requested time window
    scheduler->>+storage: get aircrafts associated with vertipads and their scheduled flights
    storage->>-scheduler: <aircrafts>, <flight_plans>
    scheduler->>scheduler: Check there are now flights scheduled for used aircraft
    Note over scheduler: get_possible_flights()
    Note over scheduler: find route from depart to arrive vertiport and cost/distance
    Note over scheduler: estimate flight time based on the distance<br>check schedules of vertiports and aircrafts<br>include deadhead legs
    Note over scheduler: produce draft itineraries
    scheduler->>scheduler: store draft plans in to memory for 30 seconds
    alt at least one flight found
        scheduler->>grpc_client: return <QueryFlightPlans>
    else no flights found
        scheduler-->>-grpc_client: return Error
    end
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
