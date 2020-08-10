# Live edits

A key feature of A/B Street is the player editing the map and seeing how traffic
responds. The possible edits include:

- Change lane types (driving, bus, bike, parking -- sidewalks are fixed)
- Change speed limits
- Reverse a lane
- Change a stop sign policy (which roads have a stop sign and which have
  priority)
- Change a traffic signal policy

The map conversion process outlined above takes a few minutes, so reusing this
process directly to compute a map with edits wouldn't work at all for real
gameplay. Instead, the process for applying edits is incremental:

- Figure out the actual diff between edits and the current map
  - This is necessary for correctness, but also speeds up a sequence of edits
    made in the UI -- only one or two lanes or intersections actually changes
    each time. Of course when loading some saved edits, lots of things might
    change.
- For any changed roads, make sure any bus stop on it have a good pointer to
  their equivalent driving position for the bus.
- For any modified intersections, recompute turns and the default intersection
  policies
- Recompute all the CHs for cars, buses, and bikes -- note sidewalks and bus
  stops never change
  - This is the slowest step. Critically, the `fast_paths` crate lets a previous
    node ordering be reused. If just a few edge weights change, then recomputing
    is much faster than starting from scratch.
  - While making edits in the UI, we don't actually need to recompute the CH
    after every little tweak. When the player exits edit mode, only then do we
    recompute everything.

A list of lanes and intersections actually modified is then returned to the
drawing layer, which uploads new geometry to the GPU accordingly.
