# Geometry-related design notes

## Intersection geometry brainstorm

- can we merge adjacent polylines at intersections based on closest angle, and then use the existing stuff to get nice geometry?
	- i think we still have to trim back correctly
	- first figure out all the trimming cases for the T, outside and inside lanes, etc


- before trimming back lines, project out the correct width. sort all those points by angle from the center. thats the intersection polygon? then somehow trim back lines to hit that nicely.
- do the current trim_lines thing, but with lines, not segments? no, there'd be many almost-parallel lines.

- at a T intersection, some lines aren't trimmed back at all

- https://www.politesi.polimi.it/bitstream/10589/112826/4/2015_10_TOPTAS.pdf pg38

- just make polygons around center lines, then intersect?






morning thoughts!

- trim lines based on outermost POLYGON border line, not lane center lines or anything
- the ascending angle and skipping existing lines in the thesis seems to make sense
- find where infinite line intersects line segment for some cases?

## Basic geometric types

Not aiming to get it right forever, just improving the mess now.

- Pt2D
	- just a pair of f64's, representing world space (non-negative)
	- no more ordered_float; have a variant only when needed
- Angle
	- normalized, with easy reversing/perpendicularing
- Line
	- pair of points
- Polyline
- Polygon

conversions to Vec2d ONLY for graphics; maybe even scope those conversions to render/

## Polylines

The polyline problem:
- https://www.codeproject.com/Articles/226569/Drawing-polylines-by-tessellation
- https://stackoverflow.com/questions/36475254/polylines-outline-construction-drawing-thick-polylines
- Will lengths change? Is this a problem?
- Drawing cars as rectangles is funky, because if their front is aligned to a new line segment, their back juts into the center of the road
- https://hal.inria.fr/hal-00907326/document
- https://www.researchgate.net/publication/220200701_High-Quality_Cartographic_Roads_on_High-Resolution_DEMs


https://wiki.openstreetmap.org/wiki/Proposed_features/Street_area


https://www.codeproject.com/Articles/226569/Drawing-polylines-by-tessellation

https://github.com/Toblerity/Shapely looks amazing! parallel_offset method calls single sided buffer in geos
	https://github.com/georust/geos bindings

https://gis.stackexchange.com/questions/1197/creating-one-sided-buffers-or-parallel-lines-in-postgis
https://trac.osgeo.org/geos/ticket/215#no1
https://geos.osgeo.org/doxygen/classgeos_1_1operation_1_1buffer_1_1BufferOp.html
