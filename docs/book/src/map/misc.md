# Development tricks

- Separate phases for fast incremental development
  - Don't reimport all data from OSM every time there's a change to part of the
    map construction code!
  - For slow steps that don't change often, make them separate binaries -- hence
    `convert_osm` being separate from the rest.
- Don't be afraid of manual intervention
  - The data isn't perfect. It's easy to spend lots of time fiddling with code
    to automatically handle all problems
  - Instead of automatically resolving problems, prefer good tooling for finding
    and specifying fixes
  - Be careful of derivative structures that could get out of sync with OSM.
    Prefer contributing real fixes to OSM.
- Screenshot diff testing
  - When working on the code for intersection geometry, it's easy to check a few
    example cases get fixed by some change. But what if another part of the map
    regresses somehow?
  - Take screenshots of the entire map, keep the checksums under version
    control, look at the diffs visually, and manually verify any changes.
  - Implementation details: One huge gif or png is too slow to read and write,
    so take a bunch of tiled screenshots covering everything. Amusingly,
    rendering to a file with `glium` is slow unless compiling in release mode
    (which isn't an option for quick incremental development). So instead, pan
    to each section of the map, render it, call an external screenshot utility,
    and move on -- just don't wiggle the mouse during this process!
- Different IDs for objects make sense during different phases
  - For the final product, lanes and such are just a contiguous array, indexed
    by numeric IDs.
  - But sometimes, we need IDs that're the same between different boundary
    polygons of maps, so that player edits can be applied anywhere. Using
    (longitude, latitude) pairs hits floating-point serialization and comparison
    issues, so referring to roads as (OSM way ID, OSM node ID 1, OSM node ID 2)
    works instead.

## Appendix: PolyLines

Add some pictures here to demonstrate how polyline shifting works, the
explode-to-infinity problem, and the bevel/miter fix.
