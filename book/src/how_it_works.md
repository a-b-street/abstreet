# How A/B Street works

The overview:

1.  A detailed map of Seattle is built from
    [OpenStreetMap (OSM)](https://www.openstreetmap.org/about)
2.  A realistic set of daily trips by car, bike, foot, and bus are simulated
3.  You make small changes to roads and intersections
4.  You explore how these changes affect the trips

Details below. Many limitations are mentioned; improvements are ongoing. I'll
add pictures to explain better when I get time.

<!--ts-->

- [How A/B Street works](#how-ab-street-works)
  - [Driving](#driving)
  - [Parking](#parking)
  - [Biking](#biking)
  - [Walking](#walking)
  - [Transit](#transit)
  - [Intersections](#intersections)
  - [People and trips](#people-and-trips)
  - [Map edits](#map-edits)

<!-- Added by: dabreegster, at: Mon Jun  8 12:17:13 PDT 2020 -->

<!--te-->

## Driving

- Movement: no acceleration, go the full speed limit of the road unless there's
  a slower vehicle in front
- Lanes
  - No over-taking or lane-changing in the middle of a road, only at
    intersections
  - Strange choice of lanes -- the least full at the time of arrival
  - Narrow two-way neighborhood roads where, in practice, only one car at a time
    can go are currently full two-way roads
- Routing is based on fastest time assuming no traffic
  - No rerouting if the driver encounters a traffic jam

## Parking

- Types
  - On-street: parallel parking lanes from
    [GeoData blockface dataset](http://data-seattlecitygis.opendata.arcgis.com/datasets/blockface)
    and [manually mapped](side_projects/parking_mapper.md)
  - Off-street: most buildings have at least a few parking spots in a driveway
    or carport
    - Currently experimenting in the downtown map: set the number of available
      spots based on number of cars seeded at midnight
  - Parking lots: the number of spots is inferred
- Restrictions
  - All spots are public except for the few spots associated with each building
  - No time restrictions or modeling of payment
- How cars park
  - Drivers won't look for parking until they first reach their destination
    building. Then they'll drive to the nearest open parking spot (magically
    knowing what spots are open, even if they're a few blocks away). If somebody
    else has taken the spot when they arrive, they'll try again.
  - Once a driver finds an open spot, they'll take 10-15 seconds to park. They
    block the road behind them in the meantime. There are no conflicts between
    pedestrians and cars when using a driveway. Cars won't make left turns into
    or out of driveways.
- Some parking along the boundary of the map is "blackholed", meaning it's
  impossible to actually reach it. Nobody will use these spots.

## Biking

- Choice of lane
  - Multi-use trails like the Burke Gilman and separated cycle-tracks like the
    one along Broadway are currently missing
  - Cyclists won't use an empty parking lane
  - On roads without a bike lane, cyclists currently won't stick to the
    rightmost lane
  - No over-taking yet, so cars can get stuck behind a bike even if there's a
    passing lane
- Elevation change isn't factored into route choice or speed yet; pretend
  everybody has an e-bike
- Beginning or ending a cycling trip takes 30-45 seconds. Locking up at bike
  racks with limited capacity isn't modeled; in practice, it's always easy in
  Seattle to find a place to lock up.

## Walking

- Not using sidewalk and crosswalk data from OSM yet
- No jay-walking, even on empty residential streets
- Pedestrians can't use roads without sidewalks at all
  - When a road only has a sidewalk on one side, driveways will cross the road
- Pedestrians can "ghost" through each other; crowds of people can grow to any
  size

## Transit

- The modeling of buses is extremely simple and buggy; I'll work on this soon
- No light rail yet

## Intersections

- Conflicting movements are coarse: a second vehicle won't start a conflicting
  turn, even if the first vehicle is physically out of the way but still
  partially in the intersection
- Most of the time, vehicles won't "block the box" -- if there's no room in the
  target lane, a vehicle won't start turning and risk getting stuck in the
  intersection
- Traffic signals
  - Only fixed timers; no actuated signals or
    [centralized control](https://www.seattle.gov/transportation/projects-and-programs/programs/technology-program/mercer-scoot)
    yet
  - The timing and stages are automatically guessed, except some intersections
    are
    [manually mapped](https://docs.google.com/document/d/1Od_7WvBVYsvpY4etRI0sKmYmZnwXMAXcJxVmm8Iwdcg/edit?usp=sharing)
  - No pedestrian beg buttons; walk signals always come on
  - The signal doesn't change for rush hour or weekday/weekend traffic; there's
    one pattern all day
- Turn restrictions from OSM are applied
  - Per lane (left turn only from leftmost lane), entire roads, multiple
    intersections

## People and trips

- A "synthetic population" of ~700,000 people come from
  [PSRC's Soundcast model](https://www.psrc.org/activity-based-travel-model-soundcast)
  - Soundcast uses census, land-use, vehicle counts, and commuter surveys. The
    current data is from 2014.
  - All driving trips are currently single-occupancy; no car-pooling or
    ridesharing
  - Parked cars are initially placed at midnight based on the number of trips
    between buildings
- Each person's schedule never changes
  - Your changes to the map won't yet convince somebody to take a bus or walk
    instead of drive

## Map edits

- Types of edits
  - Change types of lanes. Sometimes this is unrealistic based on actual road
    width, but data for this is unavailable.
  - Reversing direction of lanes
  - Changing stop signs
  - Changing traffic signal timing
  - Closing roads and intersections for construction, forcing rerouting
- Disconnecting the map
  - Generally you can't close sidewalks or make changes to make buildings
    unreachable
  - You shouldn't be able to make bus stops unreachable, but currently this is
    buggy
