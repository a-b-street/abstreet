# A/B Street's map model as a platform

A/B Street's representation of a city, built mostly from OSM and lots of
heuristics, is likely useful to other projects. This doc brainstorms what it
would look like to properly expose it to other users.

To sum up what the map model provides: geometry + semantics.

## Use cases

- Different UIs (particularly 3D / VR) for exploring cities as they are or as
  they could be, like Streetmix 3D and Complete Street Rule
- Importing slices of a city as assets into a game engine like Godot
  - Imagine a hackathon where people easily build games based on the real world
  - Like <https://developers.google.com/maps/documentation/gaming/overview_musk>
    but open
- A new OSM viewer/editor

TODO: Give a quick Python example of what interacting with the end goal could
look like.

## Just data is not enough

At first glance, the existing `Map` structure could be written to some format
with a nicely documented schema. This would certainly be useful, but it's not
nearly enough. Interpreting the data sometimes requires lots of code, which
already exists -- so why not expose it to users as well?

Examples in OSM where I wish "standard libraries" existed to interpret the data:

- The simple task of detecting intersections between ways
- [Figuring out what lanes a road has from tags](https://github.com/dabreegster/abstreet/blob/master/map_model/src/make/initial/lane_specs.rs)
- Gluing multipolygons together
- Inferring turns at an intersection, subject to the several types of turn
  restrictions

A/B Street solves these problems (or at least it tries to), but by itself, the
resulting data isn't always useful. So some examples of where a library would be
needed too:

- Pathfinding. ABST does lots of work especially to handle "live" map edits and
  cheaply regenerate contraction hierarchies. Also, pathfinding requires obeying
  OSM turn restrictions that span multiple roads -- this prevents even plain old
  Dijkstra's from working correctly.
- Getting geometry in different forms. Lanes are stored as a `PolyLine`, but
  what if a consumer wants the thickened `Polygon`, either as points, or maybe
  even pre-triangulated vertices and indices?

## How would an API/library work?

The traditional approach is to link against part of A/B Street as a library and
call it through language-specific bindings. The more language-agnostic option is
defining an API (maybe JSON or protobuf) and having clients run a local A/B
Street server, making HTTP requests to it. This is like the "sidecar" pattern in
microservice-land.

## Compatibility

Really have to think through this carefully. Some examples of big changes on the
horizon:

- Additive: separate cycleways and tramways. Likely no schema change.
- Modify: traffic signals will get
  [more complex](https://github.com/dabreegster/abstreet/issues/295)
- Modify: we'll likely try again to merge tiny intersections together, which
  would get rid of the current guarantees that a road/intersection is associated
  to one particular OSM object

## Layering

Clients should be able to opt into different data layers. For example, A/B
Street strips out OSM building tags right now to keep filesizes small. But an
OSM viewer would want to keep this (and likely discard the large contraction
hierarchies). So some pieces of the map model need to be teased apart into
optional pieces, and probably loaded in as separate files.

## The bigger vision

Depending what other open source projects are on board, the general idea is to
start assembling an ecosystem of libraries/tooling to make it easier to build
new things off of open GIS data.
