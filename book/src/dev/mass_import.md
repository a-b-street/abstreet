# Mass importing many maps

For <https://github.com/dabreegster/abstreet/issues/326>, I'm starting to figure
out how to import hundreds of maps into A/B Street. There are many issues with
scaling up the number of supported maps. This document just focuses on
importing.

## The current approach

<https://download.bbbike.org/> conveniently has 200 OSM extracts for major
cities world-wide. The `data/bbike.sh` script downloads these. Then
`data/mass_import.sh` attempts to import them into A/B Street.

The bbike extracts, however, cover huge areas surrounding major cities.
Importing such large areas is slow, and the result is too large to work well in
A/B Street or the OSM viewer. Ideally, we want just the area concentrated around
the "core" of each city.

<https://github.com/dabreegster/abstreet/blob/master/convert_osm/src/bin/extract_cities.rs>
transforms a huge .osm file into smaller pieces, each focusing on one city core.
This tool looks for administrative boundary relations tagged as cities, produces
a clipping polygon covering the city, and uses `osmconvert` to produce a smaller
`.osm` file. The tool has two strategies for generating clipping polygons. One
is to locate the `admin_centre` or `label` node for the region, then generate a
circle of fixed radius around that point. Usually this node is located in the
city core, so it works reasonably, except for "narrow" cities along a coast. The
other strategy glues together the relation's multipolygon boundary, then
simplifies the shape (usually with thousands of points) using a convex hull.
This strategy tends to produce results that're too large, because city limits
are often really huge.

## Problems

- Outside the US, administrative boundaries don't always have a "city" defined.
  In Tokyo in particular, this name isn't used. I'm not sure which boundary
  level to use yet.
- The tool assumes driving on the right everywhere. OSM has
  <https://wiki.openstreetmap.org/wiki/Key:driving_side>, but this is usually
  tagged at the country level, which isn't included in the bbike extracts.
- The resulting maps are all "flattened" in A/B Street's list, so you can't see
  any hierarchy of areas. Two cities with the same name from different areas
  will arbitrarily collide.
