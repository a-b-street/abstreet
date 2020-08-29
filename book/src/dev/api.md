# API

Suppose you're tired of manually fiddling with traffic signals, and you want to
use machine learning to do it. You can run A/B Street without graphics and
automatically control it through an API.

## Example

This
[Python example](https://github.com/dabreegster/abstreet/blob/master/headless/examples/python_client.py)
has everything you need to get started.

## API details

> **Under construction**: The API will keep changing. There are no backwards
> compatibility guarantees yet. Please make sure I know about your project, so I
> don't break your client code.

For now, the API is JSON over HTTP. The exact format is unspecified, error codes
are missing, etc. A summary of the commands available so far:

- **/sim**
  - **GET /sim/reset**: Reset all map edits and the simulation state. The trips
    that will run don't change; they're determined by the scenario file you
    initially pass to `headless`.
  - **GET /sim/get-time**: Returns the current simulation time.
  - **GET /sim/goto-time?t=06:30:00**: Simulate until 6:30 AM. If the time you
    specify is before the current time, you have to call **/sim/reset** first.
- **/traffic-signals**
  - **GET /traffic-signals/get?id=42**: Returns the traffic signal of
    intersection #42 in JSON.
  - **POST /traffic-signals/set**: The POST body must be a traffic signal in
    JSON format.
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
- **/map**
  - **GET /map/get-edits**: Returns the current map edits in JSON. You can save
    this to a file in `data/player/edits/map_name/` and later use it in-game
    normally. You can also later run the `headless` server with
    `--edits=name_of_edits`.

## Related tools

There's no API to create trips. Instead, you can
[import trips from your own data](https://dabreegster.github.io/abstreet/trafficsim/travel_demand.html#custom-import).

If you need to deeply inspect the map, you can dump it to JSON:

```
cargo run --bin iotool -- dump_map --map=data/system/maps/montlake.bin
```

The format of the map isn't well-documented yet. See the
[generated API docs](https://dabreegster.github.io/abstreet/rustdoc/map_model/index.html)
and [the map model docs](https://dabreegster.github.io/abstreet/map/index.html)
in the meantime.
