# Map model

A/B Street builds a rich representation of a city map using OpenStreetMap (OSM) and other sources. This chapter describes how.

TODO: Integrate pictures from
[these slides](https://docs.google.com/presentation/d/1cF7qFtjAzkXL_r62CjxBvgQnLvuQ9I2WTE2iX_5tMCY/edit?usp=sharing).

[This recorded presentation](https://youtu.be/chYd5I-5oyc?t=439) covers some of
this.

## The map

A single city is broken down into different pieces...

A/B Street comes with a few maps, each defined by a bounding/clipping polygon
for some portion of Seattle. Each map has these objects:

- **Roads**: A single road connects two intersections, carrying OSM metadata and
  containing some child lanes.
- **Lanes**: An individual lane of traffic. Driving (any vehicle), bus-only, and
  bike-only lanes have a direction. On-street parking lanes don't allow any
  movement, and they have some number of parking spots. Sidewalks are
  bidirectional.
- **Intersections**: An intersection has references to all of the incoming and
  outgoing lanes. Most intersections have a stop sign or traffic signal policy
  controlling movement through it.
  - **Border** intersections on the edge of the map are special places where
    agents may appear or disappear.
- **Turns**: A turn connects one lane to another, via some intersection.
  (Sidewalks are bidirectional, so specifying the intersection is necessary to
  distinguish crosswalks at each end of a sidewalk.)
- **Buildings**: A building has a position, OSM metadata, and a **front path**
  connecting the edge of the building to the nearest sidewalk. Most trips in A/B
  Street begin and end at buildings. Some buildings also contain a number of
  off-street parking spots.
- **Area**: An area has geometry and OSM metadata and represents a body of
  water, forest, park, etc. They're just used for drawing.
- **Bus stop**: A bus stop is placed some distance along a sidewalk, with a
  pointer to the position on the adjacent driving or bus lane where a bus stops
  for pick-up.
- **Bus route**: A bus route has a name and a list of stops that buses will
  cycle between. In the future, they'll include information about the
  frequency/schedule of the route.
- **Parking lot**: A parking lot is connected to a road, has a shape, and has
  some internal driving "aisles." The number and position of individual parking
  spots is auto-generated.

## Coordinate system

A/B Street converts (longitude, latitude) coordinates into a simpler form.

- An (x, y) point starts with the top-left of the bounding polygon as the
  origin. Note this is screen drawing order, not a Cartesian plane (with Y
  increasing upwards) -- so angle calculations account for this.
- The (x, y) values are f64's trimmed to a few decimal places, with way more
  precision than is really needed. These might become actual fixed-point
  integers later, but for now, a `Pt2D` skirts around Rust's limits on f64's by
  guaranteeing no NaN's or infinities and thus providing the full `Eq` trait.
- A few places in map conversion compare points using different thresholds,
  usually below 1 meter. Ideally these epsilon comparisons could be eliminated
  in favor of a fixed-point integer representation, but for now, explicit
  thresholds are useful.

## Invariants

Ideally, the finalized maps would satisfy a list of invariants, simplifying the
traffic simulation and drawing code built on top. But the input data is quite
messy and for now, most of these aren't quite guaranteed to be true.

- Some minimum length for lanes and turns. Very small lanes can't be drawn, tend
  to break intersection polygons, and may lead to gridlocked traffic.
- Some guarantees that positions along adjacent lanes actually match up, even
  though different lanes on the same road may have different lengths. Examples
  include the position of a bus stop on the sidewalk and bus lane matching up.
  - Additionally, parking lanes without an adjacent driving lane or bus stops
    without any driving or bus lanes make no sense and should never occur.
- Connectivity -- any sidewalk should be reachable from any other, and most
  driving lanes should be accessible from any others. There are exceptions due
  to border intersections -- if a car spawns on a highway along the border of
  the map, it may be forced to disappear on the opposite border of the map, if
  the highway happens to not have any exits within the map boundary.
