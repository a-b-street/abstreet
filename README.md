# A/B Street

Ever been on a bus stuck in traffic, wondering why there are cars parked on the
road instead of a bus lane? A/B Street is a game exploring how small changes to
a city affect the movement of drivers, cyclists, transit users, and pedestrians.

[Play A/B Street on Windows, Mac, Linux](docs/INSTRUCTIONS.md)

Removing dedicated left-turn cycles from a traffic signal:

![fix_traffic_signal](docs/videos/fix_traffic_signal.gif)

Watching overall traffic patterns and zooming into a few slow areas:

![exploring_traffic](docs/videos/exploring_traffic.gif)

## Technical articles

- [Map model](docs/articles/map/article.md)
- [Traffic simulation](docs/articles/trafficsim/article.md)
- [Rust implementation notes](docs/articles/rust/article.md)
- [Running A/B Street in a new city](docs/new_city.md)
- [Features (outdated)](docs/articles/features/article.md)

## Features

- The map
  - A detailed rendering of Seattle from OpenStreetMap and King County GIS data,
    including sidewalks, on-street parking, bike lanes, bus-only lanes, turn
    lanes, buildings, and bus stops.
  - Intersections governed by stop signs and traffic signals, with default
    signal timings heuristically inferred. Hand-tuned geometry to reasonably
    model Seattle's strangest intersections.
  - You can adjust lane types, stop signs, and traffic signals.
- The traffic
  - Individual cars, buses, bikes, and pedestrians move through the map.
  - Most trips are multi-modal -- for example, a pedestrian exits a building,
    walks a few blocks over to their parked car, drives somewhere, looks for
    parking, and walks to their final destination.
  - A realistic set of trips -- how many people go from building 1 to building 2
    at some time using some form of transport -- comes from
    [PSRC's Soundcast](https://www.psrc.org/activity-based-travel-model-soundcast)
    model.
- The gameplay
  - Start in sandbox mode, exploring the map, watching traffic patterns,
    following individual agents, looking for problems.
  - Jump to edit mode, where you can convert some on-street parking to bus lanes
    and adjust traffic signals to try to fix some problem.
  - Try your change in A/B test mode, running two traffic simulations
    side-by-side. Explore how individual agents finish their trips faster or
    slower, and compare aggregate results about different groups of traffic.

## Project mission

If you fix some traffic problem while playing A/B Street, my ultimate goal is
for your changes to become a real proposal for adjusting Seattle's
infrastructure. A/B Street is of course a game, using a simplified approach to
traffic modeling, so city governments still have to evaluate proposals using
their existing methods. A/B Street is intended as a conversation starter and
tool to communicate ideas with interactive visualizations.

Why not leave city planning to professionals? People are local experts on the
small slice of the city they interact with daily -- the one left turn lane that
always backs up or a certain set of poorly timed walk signals.
[Laura Adler](http://www.govtech.com/data/SimCities-Can-City-Planning-Mistakes-Be-Avoided-Through-Data-Driven-Simulations.html)
writes:

> "Only with simple, accessible simulation programs can citizens become active
> generators of their own urban visions, not just passive recipients of options
> laid out by government officials."

Existing urban planning software is either proprietary or hard to use. A/B
Street strives to set the accessibility bar high, by being a fun, engaging game.

## Contributing

I'm a one-person team. If you want to bring this to your city or if you're
skilled in user experience design, traffic simulation, data visualization, or
civic/government outreach, please contact Dustin Carlino at
<dabreegster@gmail.com> or post at
[r/abstreet](https://www.reddit.com/r/abstreet/). I also welcome any
contributions on [Patreon](https://www.patreon.com/abstreet).
