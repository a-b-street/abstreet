# Brainstorm about lanes

It's time to model more things:

- lanes with parked cars
- bike lanes
- sidewalks

These have a geometric aspect (which seems similar / derivable the same way as
roads) and a graph / usage aspect.

Where should the expansion of roads into lanes happen? In the initial map
conversion, adding an index and type to the current road proto? Or better, in
map_model::new, to defer it a bit.

And actually, there isn't really much use in having roads and lanes. Get rid of
the concept of road after the proto.
