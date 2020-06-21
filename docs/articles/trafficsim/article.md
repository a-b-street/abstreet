# A/B Street's Traffic Simulation

This article describes how cars, bikes, buses, and pedestrians are modeled in
A/B Street. All code lives in the `sim` crate. This is up-to-date as of
July 2019. Since then, the main change is some gridlock resolution that I've yet
to describe.

[This recorded presentation](https://youtu.be/chYd5I-5oyc?t=1086) covers some of
this.

The traffic simulation models different agents (cars, bikes, buses, pedestrians,
and intersections) over time. Agents don't constantly sense and react to the
world every second; instead, they remain in some state until something
interesting happens. This is a discrete-event architecture -- events are
scheduled for some time in the future, and handling them changes the state of
some agents. The core simulation loop simply processes events in order -- see
`scheduler.rs` and the `step` method in `sim.rs`.

<!--ts-->

- [A/B Street's Traffic Simulation](#ab-streets-traffic-simulation)
  - [Discrete-event simulation](#discrete-event-simulation)
    - [Cars](#cars)
      - [Exact positions](#exact-positions)
    - [Lane-changing](#lane-changing)
    - [Pedestrians](#pedestrians)
    - [Intersections](#intersections)
  - [Demand data](#demand-data)
  - [Appendix: discrete-time simulation](#appendix-discrete-time-simulation)

<!-- Added by: dabreegster, at: Wed Jul 10 12:06:04 BST 2019 -->

<!--te-->

## Discrete-event simulation

### Cars

(Note: Cars, bikes, and buses are all modeled the same way -- bikes just have a
max speed, and buses/bikes can use restricted lanes.)

Cars move through a sequence of lanes and turns (movements through an
intersection). They queue and can't over-take a slow lead vehicle. The main
simplifying assumption in A/B Street is that cars can instantly accelerate and
decelerate. This wouldn't model highway driving at all, where things like jam
waves are important, but it's reasonable for in-city driving. The essence of
scarcity is the capacity on lanes and the contention at intersections. What
happens in between isn't vital to get exactly right.

A car has a few states (`mechanics/car.rs`):

- **Crossing** some distance of a lane/turn over some time interval
- **Queued** behind another car on a lane/turn
- **WaitingToAdvance** at the end of a lane, blocked on an intersection
- A few states where the car stays in one place: **Parking**, **Unparking**, and
  **Idling** (for buses at a stop)

State transitions happen in `mechanics/driving.rs`. This is best explained by an
example sequence:

- A car enters the Unparking state, taking a fixed 30s to exit a parking spot
  and enter the adjacent driving lane. The driving lane is blocked during this
  time, to mimic somebody pulling out from a parallel parking spot.
- The car is now fully somewhere on the driving lane. It enters the Crossing
  state, covering the remaining distance to the end of the road. The time
  interval is calculated assuming the car travels at the max speed limit of the
  road.
- After that time, the car checks if there's anybody in the queue before it.
  Nope? Then it attempts to initiate a turn through the intersection, but the
  stop sign says no, so the car enters the WaitingToAdvance state.
- Some time later, the stop sign wakes up the car. The car starts the turn,
  entering the Crossing state again.
- After finishing the turn, the car starts Crossing the next lane. When it's
  finished, it turns out there are a few cars ahead of it, so it enters the
  Queued state.
- When the lead vehicle directly in front of the car exits the lane, it wakes up
  the car, putting it in the Crossing state, starting at the appropriate
  following distance behind the lead vehicle. This prevents the car from
  immediately warping to the end of the lane when the lead vehicle is out of the
  way.
- And so on...

#### Exact positions

For a discrete-event simulation, we don't usually care exactly where on a lane a
car is at some time. But we do need to know for drawing and for a few cases
during simulation, such as determining when a bus is lined up with a bus stop in
the middle of a lane. `mechanics/queue.rs` handles this, computing the distance
of every car in a lane. For cars in the `Crossing` state, we linearly
interpolate distance based on the current time. Of course, cars have to remain
in order, so Queued cars are limited by the lead vehicle's position + the lead
vehicle's length + a fixed following distance of 1m.

Another case where we need to know exact positions of cars is to prevent the
first vehicle on a lane from hitting the back of a car who just left the lane.
All vehicles have length, and position is tracked by the front of the car. When
a car's front leaves a lane, its back is still partly in the lane. Logically,
the new lead car in the lane still needs to act like it's Queued. So each lane
keeps a "laggy head", pointing to the car with its back partly in the lane.
After the laggy head has made it sufficient distance along its new turn or lane,
the laggy head on the old lane can be erased, unblocking the lead vehicle. This
requires calculating exact distances and some occasionally expensive cases where
we have to schedule frequent events to check when a laggy head is clear.

### Lane-changing

Lane-changing (LCing) deserves special mention. A/B Street cheats by not
allowing it on lanes themselves. Instead, at intersections, cars can perform
turns that shift them over any number of lanes. These LCing turns conflict with
other turns appropriately, so the contention is still modeled. Why do it this
way? In a
[previous project](http://apps.cs.utexas.edu/tech_reports/reports/tr/TR-2157.pdf),
I tried opportunistic LCing. If a car had room to warp to the equivalent
distance on the adjacent lane without causing a crash, it would start LCing,
then take a fixed time to slide over, blocking both lanes throughout. This meant
cars often failed to LC when they needed to, forcing them to reroute, botching
their trip times. In many cases the cars would be permanently stuck, because
pathfinding would return paths requiring LCing that couldn't be pulled off in
practice due to really short roads. Why not try making the car slow down if
needed? Eventually it might have to stop, which could lead to unrealistic
gridlock. This LCing model was using a detailed discrete-time model with cars
accelerating properly; maybe it's easier with A/B Street's simplified movement
model.

Currently in A/B Street, cars will pick the least backed-up lane when there's a
choice. They make this decision once when they reach the front of a queue; look
for `opportunistically_lanechange` in `router.rs`. The decision could be
improved.

### Pedestrians

Pedestrian modeling -- in `mechanics/walking.rs` is way simpler. Pedestrians
don't need to queue on sidewalks; they can "ghost" through each other. In
Seattle, there aren't huge crowds of people walking and slowing down, except for
niche cases like Pike Place Market. So in A/B Street, the only scarce resource
modeled is the time spent waiting to cross intersections.

### Intersections

I need to flesh this section out. See `mechanics/intersections.rs` for how stop
signs and traffic signals work. Two things I need to fix before making this
section interesting:

- Only wake up relevant agents when a previous agent finishes a turn.
- Don't let an agent start a low-priority turn (like an unprotected left) if
  it'll force a high-priority vehicle approaching to wait. The approaching
  vehicle is still in the Crossing state, so we need to notify intersections
  ahead of time of intended turns and an ETA.

One contributor to permanent gridlock is cars and bikes being stuck in an
intersection, preventing conflicting turns from being performed. To help avoid
this, one of the last checks that stop signs and traffic signals perform before
accepting a new turn request is that the target lane has enough space for the
new vehicle. This is "reserved" space, not necessarily currently occupied by
vehicles in that lane. This accounts for other vehicles performing a turn bound
for that lane. See `try_to_reserve_entry` in `mechanics/queue.rs`. When a car
completely leaves a lane (determined by the "laggy head" described above), this
space is freed, and blocked cars are woken up.

## Demand data

The input to a traffic simulation consists of a list of trips -- start from
building 1 at some time, and travel to building 2 via walking/driving/bus/bike.
How do we generate a realistic set of trips to capture Seatle's traffic
patterns? Picking origins and destinations uniformly at random yields extremely
unrealistic results. One approach is to harvest location data from phones -- but
this is expensive and invasive to privacy. Another approach is to generate a
synthetic population based on census data, land-use of different buildings,
vehicle counts, travel surveys, and such. In Seattle, the Puget Sound Regional
Council (PSRC) uses the
[Soundcast model](https://www.psrc.org/activity-based-travel-model-soundcast) to
do exactly this.

A/B Street imports data from PSRC using the `popdat` crate (the canonically
trendy rendering of "population data"). This is further processed in
`game/src/mission/trips.rs`.

## Appendix: discrete-time simulation

A/B Street's first traffic model was discrete-time, meaning that every agent
reacted to the world every 0.1s. Cars had a more realistic kinematics model,
accelerating to change speed and gradually come to a halt. Cars did a worst-case
estimation of how far ahead they need to lookahead in order to satisfy different
constraints:

- Don't exceed any speed limits
- Don't hit the lead vehicle (which might suddenly slam on its brakes)
- Stop at the end of a lane, unless the intersection says to go

After fighting with this approach for a long time, I eventually scrapped it in
favor of the simpler discrete-event model because:

- It's fundamentally slow; there's lots of busy work where cars in freeflow with
  nothing blocking them or stopped in a long queue constantly check to see if
  anything has changed.
- Figuring out the acceleration to apply for the next 0.1s in order to satisfy
  all of the constraints is really complicated. Floating point inaccuracies
  cause ugly edge cases with speeds that wind up slightly negative and with cars
  coming to a complete stop slightly past the end of a lane. I wound up storing
  the "intent" of an action to auto-correct these errors.
- The realism of having cars accelerate smoothly didn't add value to the core
  idea in A/B Street, which is to model points of contention like parking
  capacity and intersections. (This is the same reason why I don't model bike
  racks for parking bikes -- in Seattle, it's never hard to find something to
  lock to -- this would be very different if Copenhagen was the target.)
  Additionally, the kinematics model made silly assumptions about driving anyway
  -- cars would smash on their accelerators and brakes as hard as possible
  within all of the constraints.
