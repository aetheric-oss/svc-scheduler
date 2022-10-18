
# Scheduler algorithm (MVP)

### MVP assumes:

- no batching of customer requests,
- single customer - single flight (no rideshare)
- no booking of recurrent flights
- aircrafts and pilots have same schedules??
- one vertiport has one pad??


### Customer input for a flight

- src location,
- dest location,
- time and date
- other constraints (weight of cargo, number of seats for passengers)

### Proposed algorithm:
```
1. Find nearest vertiports for src and dest in radius
- try 1km, if none found then 2km then 5km etc.

2. Sort vertiport src/dest combinations - best has lowest combined distance

3. For each src/dest vertiport combination
- Router to find routes and discard combinations which are unreachable
- Calculate approximate time needed from boarding/loading to landing/unloading

4. Filter Vertiports based on availability for date and time
- check generic schedule of the vertiport
- fetch all draft and confirmed flight plans connected to this vertiport
(EXIT) if no src/dest vertiport pair is found

5. Check available pilots/aircrafts for all found vertiport schedules
- fetch pilots/aircrafts operating in the area of vertiports
- check generic schedule of the pilot/aircraft
- fetch all draft and confirmed flight plans connected to this pilot/aircraft
(EXIT) if no pilot/aircraft is found

6. Check other constraints (cargo weight, number of passenger seats)

7. Create draft flight plan with linked v/p/a.

8. Schedule auto-unblocking if flight plan not confirmed by user (e.g. 30 seconds timer)
- update flight plan to cancelled
```


###Schedule representation

- calendar for each vertiport/pad/pilot/aircraft with recurring events (blocked times)
- calendar starts with standard schedule (working hours) - having events as blocked time and available as no event
    - individual flights create one-off blocking events

We want to query if vertiport is available for the period of take-off/landing time
We want to query if pilot/aircraft is available during the whole flight

Possible implementation using rrule crate for recurring events and flight_plans for booked flights:

Each v/p/p/a entity in database can have a TEXT field schedule with RRULE string
- see https://docs.rs/rrule/0.10.0/rrule/ and referenced iCalendar RFC
- this is useful to capture schedule including working hours, lunch breaks, maintenance windows, public holidays and all recurring disruptions to availability
- Step 1 of querying availability - we will need to check recurring events first

Every created Flight plan (draft or confirmed) is linked to v/p/p/a and has a start and end date
- Step 2 of querying availability - we will check if for the proposed duration there are no flight plans already associated with given v/p/p/a


If both queries return zero blocking events/flights, then we can use the time slot for the proposed draft flight plan.**
