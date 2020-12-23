# Travel demand

A/B Street simulates people following a schedule of trips over a day. A single
_trip_ has a start and endpoint, a departure time, and a mode. Most trips go
between buildings, but the start or endpoint may also be a border intersection
to represent something outside the map boundaries. The mode specifies whether
the person will walk, bike, drive, or use transit. Without a good set of people
and trips, evaluating some changes to a map is hard -- what if the traffic
patterns near the change aren't realistic to begin with? This chapter describes
where the travel demand data comes from.

## Scenarios

A _scenario_ encodes the people and trips taken over a day. See the
[code](https://github.com/dabreegster/abstreet/blob/master/sim/src/make/scenario.rs).

TODO:

- talk about vehicle assignment / parked car seeding

## Data sources

### Seattle: Soundcast

Seattle luckily has the Puget Sound Regional Council, which has produced the
[Soundcast model](https://www.psrc.org/activity-based-travel-model-soundcast).
They use census stats, land parcel records, observed vehicle counts, travel
diaries, and lots of other things I don't understand to produce a detailed model
of the region. We're currently using their 2014 model; the 2018 one should be
available sometime in 2020. See the
[code](https://github.com/dabreegster/abstreet/tree/master/importer/src/soundcast)
for importing their data.

TODO:

- talk about how trips beginning/ending off-map are handled

### Berlin

This work is [ongoing](https://github.com/dabreegster/abstreet/issues/119). See
the
[code](https://github.com/dabreegster/abstreet/blob/master/importer/src/berlin.rs).
So far, we've found a population count per planning area and are randomly
distributing the number of residents to all residential buildings in each area.

### Proletariat robot

What if we just want to generate a reasonable model without any city-specific
data? One of the simplest approaches is just to spawn people beginning at
residential buildings, make them go to some workplace in the morning, then
return in the evening. OpenStreetMap building tags can be used to roughly
classify building types and distinguish small houses from large apartments. See
the `proletariat_robot`
[code](https://github.com/dabreegster/abstreet/blob/master/sim/src/make/activity_model.rs)
for an implementation of this.

This is [ongoing](https://github.com/dabreegster/abstreet/issues/154) work
spearheaded by Mateusz. Some of the ideas for next steps are to generate
different types of people (students, workers), give them a set of activities
with durations (go to school for 7 hours, 1 hour lunch break), and then further
pick specfic buildings to travel to using more OSM tags.

### Census Based

Trips are distributed based on where we believe people live. For the US, this
information comes from the US Census. To take advantage of this model for areas
outside the US, you'll need to add your data to the global `population_areas`
file. This is one huge file that is shared across regions. This is more work up
front, but makes adding individual cities relatively straight forward.

#### Preparing the `population_areas` file

See `popdat/scripts/build_population_areas.sh` for updating or adding to the
existing population areas. Once rebuilt, you'll need to upload the file so that
popdat can find it.

### Custom import

If you have your own data, you can import it. The input format is JSON -- an
example:

```
{
  "scenario_name": "monday",
  "people": [
    {
      "origin": {
        "Position": {
          "longitude": -122.303723,
          "latitude": 47.6372834
        }
      },
      "trips": [
        {
          "departure": 10800.0,
          "destination": {
            "Position": {
              "longitude": -122.3075948,
              "latitude": 47.6394773
	    }
          },
          "mode": "Drive"
        }
      ]
    }
  ]
}
```

Run the tool:

```
cargo run --bin import_traffic -- --map=data/system/seattle/maps/montlake.bin --input=/path/to/input.json
```

The tool matches input positions to the nearest building or border intersection,
within 100 meters. The `departure` time is seconds since midnight. The tool will
fail if any point doesn't match to a building, or if any of the specified trips
can't be created (due to graph connectivity problems, for example). If your
requirements are different or you have any trouble using this format/tool,
please file a Github issue -- just consider this tool and format a prototype.

## Modifying demand

The travel demand model is extremely fixed; the main effect of a different
random number seed is currently to initially place parked cars in specific
spots. When the player makes changes to the map, exactly the same people and
trips are simulated, and we just measure how trip time changes. This is a very
short-term prediction. If it becomes much more convenient to bike or bus
somewhere, then more people will do it over time. How can we transform the
original demand model to respond to these changes?

Right now, there's very preliminary work in sandbox mode for Seattle weekday
scenarios. You can cancel all trips for some people (simulating lockdown) or
modify the mode for some people (change 50% of all driving trips between 7 and
9am to use transit).

## Research

- <https://github.com/replicahq/doppelganger>
- <https://github.com/stasmix/popsynth>
- <https://zephyrtransport.github.io/zephyr-directory/projects/>
- <https://activitysim.github.io>
- <https://github.com/BayAreaMetro/travel-model-one>
- <https://github.com/RSGInc/DaySim>
- <https://github.com/arup-group/pam>
- <https://spatial-microsim-book.robinlovelace.net/smsimr.html>
- <https://github.com/DLR-VF/TAPAS>
- <https://sumo.dlr.de/docs/Demand/Activity-based_Demand_Generation.html>
