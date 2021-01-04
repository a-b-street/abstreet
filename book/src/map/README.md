# Map model

A/B Street transforms OpenStreetMap (OSM) data into a detailed geometric and
semantic representation of the world for traffic simulation. This chapter
describes that map model, with the hopes that it'll be useful for purposes
beyond this project.

## Overview

A `Map` covers everything inside some hand-drawn boundary, usually scoped to a
city or a few of a city's districts. Unlike OSM, it doesn't cover the entire
world; it only has areas specifically extracted for some purpose.

A map consists of many objects. Mainly, there are roads, broken down into
individual lanes, and intersections. A road is a single segment connecting
exactly two intersections (as opposed to OSM, where a single "way" may span many
intersections). Lanes within a road have a specific type, which dictates their
direction of travel (or lack of travel, like on-street parking) and uses.
Sidewalks are represented as bidirectional lanes. Roads connect at
intersections, which contain an explicit set of turns, each linking a source
lane to a destination lane.

Maps also contain parking lots and buildings, which connect to the nearest
driveable lane and a sidewalk. Maps have water and park areas, only used for
drawing. They also represent public transit stops and routes.

## How is a map used?

Unlike some GIS systems, maps don't use any kind of database -- they're just a
file, anywhere from 1 to ~500MB (depending on the size of their boundary). Once
loaded into memory, different objects from the map can be accessed directly,
along with a large API to perform various queries.

Most of the map's API is read-only; once built, a map doesn't change until
user-created edits are applied.

The pipeline to import a map from OSM data (and also optional supplementary,
city-specific data) is complex and may take a few minutes to run, but it happens
once offline. Applications using maps just read the final file.

## Features

Why use A/B Street's map model instead of processing OSM directly?

TODO: Order these better. For each one, show before/after pictures

### Area clipping

Bodies of water, forests, parks, and other areas are represented in OSM as
relations, requiring the user to stitch together multiple polylines in undefined
orders and handle inner holes. A/B Street maps handle all of that, and also clip
the area's polygon to the boundary of the entire map -- including coastlines.

### Road and intersection geometry

OSM represents roads as a polyline of the physical center of the road. A/B
Street infers the number and type of lanes from OSM metadata, then creates
individual lanes of appropriate width, each with a center-line and polygon for
geometry. At intersections, the roads and lanes are "trimmed back" to avoid
overlapping, and the "common area" becomes the intersection's polygon. This
heuristic process is reasonably robust to complex shapes, with special treatment
of highway on/off-ramps, although it does still have some bugs.

### Turns

At each intersection, A/B Street infers all legal movements between vehicle
lanes and sidewalks. This process makes use of OSM metadata about turn lanes,
inferring reasonable defaults for multi-lane roads. OSM turn restriction
relations, which may span a sequence of several roads to describe U-turns around
complex intersections, are also used.

### Parking lots

OSM models parking lots as areas along with the driveable aisles. Usually the
capacity of a lot isn't tagged. A/B Street automatically fills paring lots with
individual stalls along the aisles, estimating the capacity just from this
geometry.

### Stop signs

At unsignalized intersections, A/B Street infers which roads have to stop, and
which have right-of-way.

### Traffic signals

OSM has no way to describe how traffic signals are configured. A/B Street models
fixed-timer signals, automatically inferring the number of stages, their
duration, and the movements that are prioritized and permitted during each
stage.

### Pathfinding

A/B Street can determine routes along lanes and turns for vehicles and
pedestrians. These routes obey OSM's turn restriction relations that span
multiple road segments. They also avoid roads that're tagged as not allowing
through-traffic, depending on the route's origin and destination and vehicle
type. The pathfinding optionally makes use of contraction hierarchies to greatly
speed up query performance, at the cost of a slower offline importing process.

### Bridge z-ordering

OSM tags bridges and tunnels, but the roads that happen to pass underneath
bridges aren't mapped. A/B Street detects these and represents the z-order for
drawing.

### Buildings

Similar to areas, A/B Street consolidates the geometry of OSM buildings, which
may be split into multiple polygons. Each building is also associated with the
nearest driveable lane and sidewalk, and metadata is used to infer a land-use
(like residential and commercial) and commercial amenities available.

### Experimental: public transit

A/B Street uses bus stops and route relations from OSM to build a model of
public transit routes. OSM makes few guarantees about how the specifics of the
route are specified, but A/B Street produces specific paths, handling clipping
to the map boundary.

... All of this isn't the case yet, but it's a WIP!

### Experimental: separated cyclepaths, tramways, and walking paths

Some cyclepaths, tram lines, and footpaths in OSM are tagged as separate ways,
with no association to a "main" road. Sometimes this is true -- they're
independent trails that only occasionally cross roads. But often they run
alongside a road. A/B Street attempts to detect these and "snap" them to the
main road as extra lanes.

... But this doesn't work yet at all.
