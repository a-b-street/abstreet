# API

Suppose you're tired of manually fiddling with traffic signals, and you want to
use machine learning to do it. You can run A/B Street without graphics and
automatically control it through an API.

## Examples

This
[Python example](https://github.com/dabreegster/abstreet/blob/master/headless/examples/python_client.py)
has everything you need to get started.

See
[all example code](https://github.com/dabreegster/abstreet/tree/master/headless/examples)
-- there are different experiments in Go and Python that automate running a
simulation, measuring some metric, and making a change to improve the metric.

## API details

> **Under construction**: The API will keep changing. There are no backwards
> compatibility guarantees yet. Please make sure I know about your project, so I
> don't break your client code.

For now, the API is JSON over HTTP. The exact format is unspecified, error codes
are missing, etc. A summary of the commands available so far:

- **/sim**
  - **GET /sim/reset**: Reset all temporary map edits and the simulation state.
    The trips that will run don't change; they're determined by the scenario
    specified by the last call to `/sim/load`. If you made live map edits using
    things like `/traffic-signals/set`, they'll be reset to the `edits` from
    `/sim/load`.
  - **POST /sim/load**: Switch the scenario being simulated, and also optionally
    sets the map edits.
  - **GET /sim/get-time**: Returns the current simulation time.
  - **GET /sim/goto-time?t=06:30:00**: Simulate until 6:30 AM. If the time you
    specify is before the current time, you have to call **/sim/reset** first.
  - **POST /sim/new-person**: The POST body must be an
    [ExternalPerson](https://dabreegster.github.io/abstreet/rustdoc/sim/struct.ExternalPerson.html)
    in JSON format.
- **/traffic-signals**
  - **GET /traffic-signals/get?id=42**: Returns the traffic signal of
    intersection #42 in JSON.
  - **POST /traffic-signals/set**: The POST body must be a
    [ControlTrafficSignal](https://dabreegster.github.io/abstreet/rustdoc/map_model/struct.ControlTrafficSignal.html)
    in JSON format.
  - **GET /traffic-signals/get-delays?id=42&t1=03:00:00&t2=03:30:00**: Returns
    the delay experienced by every agent passing through intersection #42 from
    3am to 3:30, grouped by direction of travel.
  - **GET /traffic-signals/get-cumulative-thruput?id=42**: Returns the number of
    agents passing through intersection #42 since midnight, grouped by direction
    of travel.
- **/data**
  - **GET /data/get-finished-trips**: Returns a JSON list of all finished trips.
    Each tuple is (time the trip finished in seconds after midnight, trip ID,
    mode, duration of trip in seconds). The mode is either a string like "Walk"
    or "Drive", or null if the trip was aborted (due to a simulation bug or
    disconnected map).
  - **GET /data/get-agent-positions**: Returns a JSON list of all active agents.
    Vehicle type (or pedestrian), person ID, and position is included.
  - **GET /data/get-road-thruput**: Returns a JSON list of (road, agent type,
    hour since midnight, throughput for that one hour period).
- **/map**
  - **GET /map/get-edits**: Returns the current map edits in JSON. You can save
    this to a file in `data/player/edits/map_name/` and later use it in-game
    normally. You can also later run the `headless` server with
    `--edits=name_of_edits`.
  - **GET /map/get-edit-road-command?id=123**: Returns an object that can be
    modified and then added to map edits.

## Working with the map model

If you need to deeply inspect the map, you can dump it to JSON:

```
cargo run --bin dump_map data/system/maps/montlake.bin > montlake.json
```

The format of the map isn't well-documented yet. See the
[generated API docs](https://dabreegster.github.io/abstreet/rustdoc/map_model/index.html)
and [the map model docs](https://dabreegster.github.io/abstreet/map/index.html)
in the meantime.

## Working with individual trips

You can use the **/sim/new-person** API in the middle of a simulation, if
needed. If possible, it's simpler to create a Scenario as input.

## Working with Scenarios

You can
[import trips from your own data](https://dabreegster.github.io/abstreet/trafficsim/travel_demand.html#custom-import).

You can also generate different variations of one of the
[demand models](https://dabreegster.github.io/abstreet/trafficsim/travel_demand.html#proletariat-robot)
by specifying an RNG seed:

```
cargo run --bin random_scenario -- --rng=123 --map=data/system/maps/montlake.bin > data/system/scenarios/montlake/home_to_work.json
```

You can also dump Scenarios (the file that defines all of the people and trips)
to JSON:

```
cargo run --bin dump_scenario data/system/scenarios/montlake/weekday.bin > montlake_weekday.json
```

You can modify the JSON, then put the file back in the appropriate directory and
use it in-game:

```
cargo run --bin game data/system/scenarios/montlake/modified_scenario.json
```

The Scenario format is also undocumented, but see the
[generated API docs](https://dabreegster.github.io/abstreet/rustdoc/sim/struct.Scenario.html)
anyway.
