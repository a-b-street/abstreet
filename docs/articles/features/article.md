# A/B Street Features

This article gives a quick overview of the features of A/B Street. A/B Street is
a traffic simulation game set in Seattle. Players explore how small changes to
road layout and intersection rules affect the movement of pedestrians, drivers,
transit, and cyclists. The game's mission is to make it fun and simple for
anybody to test an idea to improve Seattle's traffic flow and, if the idea works
well, to communicate it to others.

TODO: up-front gif showing different stuff

A/B Street is not yet generally playable (but
[if you want to anyway](/docs/INSTRUCTIONS.md)...):

- The user interface to explore and edit the map is quite clunky.
- The pieces of the game -- editing the map, running a simulation, comparing
  results -- exist, but nothing is tied together yet in a game-like format.
- Data sources describing a realistic set of trips is missing; cars start and
  end at uniformly chosen places.
- Some important things aren't yet modeled: light rail, big bike trails like the
  Burke Gilman, ridesharing services.

If you're interested in joining me and working on problems like these, please
get in touch. Funding is available. I also have half-finished articles with
technical details about how A/B Street works; just ask me to finish them.
Contact Dustin Carlino at `dabreegster@gmail.com`.

TODO: TOC?

## Map

A/B Street generates a detailed map of Seattle from OpenStreetMap (OSM), King
County GIS, and a few other sources. It takes lots of processing to make a map
suitable for simulating traffic and that's visually appealing for a game.

The portion of the code-base to transform and clean up the map are separate from
the traffic simulation. If you see another use for this map, contact me and
we'll figure out a format to export the data for your purposes. The code isn't
Seattle-specific; most things work if you only feed in OpenStreetMap data, and
plugging in another city's custom GIS data is probably not hard.

### Lanes

OSM models entire roads (crossing many intersections) coarsely, sometimes with
some metadata about lane restrictions.

![OSM](screenshots/lanes_osm.gif)

A/B Street breaks roads down into indidual lanes, automatically finding the
geometry from the OSM road's center-line. Lane types and the number of lanes
come from heuristics on the OSM metadata and from extra King County GIS
shapefiles.

- Regular driving lanes, usable by any vehicle
- Sidewalks for pedestrian movement, including bus stops and paths to buildings
- Bus- and bike-only lanes
- On-street parking lanes, with individual parking spots

![A/B Street](screenshots/lanes_abst.gif)

### Intersections (geometry)

OSM doesn't explicitly model intersections at all; some roads just share points.

![OSM](screenshots/intersections_osm.gif)

In A/B Street, lanes and intersections have disjoint geometry.

![A/B Street](screenshots/intersections_abst.gif)

This means that cars and pedestrians stop and queue at the correct position
before crossing an intersection.

![A/B Street](screenshots/moving_through_intersection.gif)

The intersection geometry is calculated automatically, even for strangely-shaped
cases.

![A/B Street](screenshots/intersection_good_geom.gif)

OSM ways often have many "intersections" very close together. These appear as
extremely short roads in A/B Street, which complicates traffic modeling.

![A/B Street](screenshots/short_roads_bridge_before.gif)

These can be merged automatically, which works reasonably well sometimes.

![A/B Street](screenshots/short_roads_bridge_after.gif)

But some cases are very complex; this is Montlake and 520 without merging short
roads:

![A/B Street](screenshots/short_roads_montlake_before.gif)

Montlake and 520 with merging doesn't look much better, so currently short road
merging is still disabled.

![A/B Street](screenshots/short_roads_montlake_after.gif)

Some highway on-ramps in OSM are modeled with particularly unusual geometry,
overlapping an arterial road.

![OSM](screenshots/highway_onramp_osm.gif)

A/B Street detects and fixes these cases.

![A/B Street](screenshots/highway_onramp_abst.gif)

### Intersections (semantics)

A/B Street models each turn through intersections, connecting an incoming lane
to an outgoing lane. Some of these turns conflict, so cars can't perform them
simultaneously. Currently stop signs and traffic signals are modeled
(roundabouts act like all-way stops).

For stop-sign controlled intersections, the bigger road by default has priority.

![A/B Street](screenshots/turns.gif)

Intersections controlled by traffic signals have a default set of timed phases.
Players can edit these.

![A/B Street](screenshots/traffic_signal.gif)

### Boundaries

How should the boundary of the map be handled? Without proper clipping, roads
and lakes go out-of-bounds, often with very strange, long roads to nowhere.

![before](screenshots/clipping_before.gif)

Proper clipping trims polygons to fit properly. Roads that cross the boundary
terminate at special border intersections, which can model traffic flowing into
or out of the map.

![after](screenshots/clipping_after.gif)

### Buildings

Light orange buildings are classified as residential, and dark orange as
commercial. Additional data from King County GIS reveals how many units some
apartments have. This could be used to generate a realistic number of trips
between residential and commercial areas.

![A/B Street](screenshots/buildings.gif)

### Editing

The player can edit the map in a few ways:

- change lane types
- change which roads stop or yield at a stop sign
- change the phases and timing of a traffic signal

These are changes that could be prototyped in real life relatively cheaply. My
goal with A/B Street is to explore improvements to Seattle that we could try
tomorrow, not longer-term improvements like light rail extensions.

## Traffic simulation

cars, buses, bikes, pedestrians - cars queue, change lanes only by turning, dont
have speed/acceleration - buses are cars that cycle between routes - bikes are
cars with a speed cap - pedestrians dont queue; in seattle, never really any
limits where that matters

agent-based model. not discrete-time/timesteps because that's slow,
discrete-event with some tricks to figure out next interesting time something
happens. that article coming soon. approximate scale

time travel, kinda working

multi-modal trips. ped starts from building, down front path, to bus stop, rides
bus, gets off, enters parked car, etc. parked cars belong to buildings.

trip generation
