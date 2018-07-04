# Brainstorm about lanes

It's time to model more things:

- multiple driving lanes, with possibly individual turn restrictions
- dedicated bus lanes
- lanes with parked cars
- bike lanes
- sidewalks

Each lane needs some geometry:

- center lines to draw agents on
	- for sidewalks, use center line to to draw agents on the left and right sides?
- polygons to draw the lane and mouseover

Open questions:

- Can we assume all lanes are the same width?
	- Seems wrong for many sidewalks especially
	- Could be wrong for bike lanes, but could just assume it's a bike lane with a buffer
- Some lanes are immutable
	- Sidewalks can't be changed to other types; they're raised with a curb

Some modeling questions:

- Where should expansion of roads into lanes happen?
	- initial OSM conversion, adding more stuff to the proto?
	- initial map_model::new loading, at least for development convenience
		- same reason that turns aren't (yet) serialized
- Is it useful to model the entire road?
	- the parent/child relation may be hard to maintain
	- but lanes need to know their siblings
	- maintaining directional sanity could be useful
	- what's the UI for changing lane types?
	- it's a bit arbitrary which lane should draw the yellow center lines



Initial design:
- "Road" becomes "Lane" with a type
- don't need to know sibling lanes yet
- arbitrarily, one lane might have extra bits/geometry for yellow center line markings
- ideally, get rid of one-wayness and original center points, and plumb along pre-shifted lines
	- but due to the polyline problem (affecting both geom center line layer that agents follow, and polygons for drawing), can't do this. encapsulate the messiness at least.
	- so, store one way and orig points and index, but have an accessor
	- as a compromise, dont interpet OSM points on a one-way road as the center, but as the edge? this is proving hard to do.

Thinking about a new design:
- Much more general "Land" primitive that's just a nice polygon boundary for drawing/selection and one (or more, for sidewalks?) center lines for how to cross the space, with a notion of turns. It's what road is now, but way simpler data.
- Maybe the GeomRoad / DrawRoad split is a little confusing after all, since the layering just isn't perfect. figure out the polygon and centerline up-front, then ditch the other intermediate gunk.
- also ideally make one polygon for the road, not a bunch of individual pieces? but then we'd have to go triangulate later for opengl anyway
- enforce that all the polygons are nonoverlapping



The polyline problem:
- https://www.codeproject.com/Articles/226569/Drawing-polylines-by-tessellation
- https://stackoverflow.com/questions/36475254/polylines-outline-construction-drawing-thick-polylines
- Will lengths change? Is this a problem?
- Drawing cars as rectangles is funky, because if their front is aligned to a new line segment, their back juts into the center of the road
- https://hal.inria.fr/hal-00907326/document
- https://www.researchgate.net/publication/220200701_High-Quality_Cartographic_Roads_on_High-Resolution_DEMs


https://wiki.openstreetmap.org/wiki/Proposed_features/Street_area


- Seemingly: line intersection of shifted lines yields the new joint point, which looks good.
	- Length increases or increases depending on the original angle and the
	  side of the road, but of course it does.
	- Width of the road varies wildly in the joint
- For drawing, round caps works nicely.




the transition is hard:
- who should be responsible for shoving road lines back to not hit intersection?
- intersection and road association is done by points... gps or not?
	- also need to retain other_side only temporarily for map construction.
	- organize map_model lib more; map construction is now very interesting
	- arguably, we could do two-phase map construction and serialize more stuff. map model is serializable because of rust magic!
	- map model kind of acts as the graph/connection layer. keep the construction more separated.
- should all GPS stuff be converted to screen at loading time? (it'd be nice to use pt2d for screen space only)
- dependency hell most easily resolved by putting polyline stuff in map_model

so really do this in two parts:
1) current structure with weird intermediate stuff, but new geometry libs
2) careful reorg

wait slow down even more -- before any of this change, lanes on adjacent roads smoosh into each other. main road doesnt, but other parts do.
- at every intersection, find corresponding lanes and trim back center lines
- do we need to do this at the level of the polygons?!



- follow aorta's multi phase map construction better.
	- polish intersection geometry
		- at a T intersection, some lines aren't trimmed back at all
		- the lane polygons overlap, even though the lines dont

	- rename Road to Lane

	- bad polygons when shifted lines invert points
		- arguably, these could be a case when there's not enough room to shift away.

	- render trees?
	- revisit parks/water (as parcels / areas, maybe)

	- big maps start centered over emptiness
	- some bldg paths are quite long.
	- make final Map serializable too
		- useful to precompute sidewalk paths
		- waiting on https://github.com/paholg/dimensioned/issues/31 to release


	- remove different colors for changed intersections
	- tune traffic light colors
	- draw stop signs as rounded ellipse (hard without using rotations in GfxCtx)


	- move map_model geometry stuff elsewhere (sim stuff also needs it though)

	- also a polygon struct? for parcels and buildings. maybe have a form that's pre-triangulated?
	- isolate vec2d

	- improve intersection geom?
		- https://www.politesi.polimi.it/bitstream/10589/112826/4/2015_10_TOPTAS.pdf
		- just make polygons around center lines, then intersect?
	- shift turn icons and stop markings and such away from crosswalk
	- figure out what to do about yellow center lines
		- yellow and white lines intersect cars and turn icons and such
		- who should own drawing them?
		- trim them back too (maybe to avoid hitting the intersection?)
		- osm tags and such would ideally be part of a master road





Crosswalk notes:
- Turns go from a src to a dst, so we'd need to double them for crosswalks, since they're always bidirectional
- Turn icons might not make sense as a UI?
- Many sidewalks directly connect at corners and shouldn't have anything drawn for them
- We don't want to draw diagonals... just from one side of the road to the other
- We want crosswalks at the beginning AND end of the sidewalk!

- v1: remember other_side for sidewalks too. draw crosswalks at the beginning AND end of every sidewalk lane.
- do extra drawing in DrawIntersection for now, figure out modeling later.




How to depict stop signs? Each driving lane has a priority... asap go or full
stop. Turns from go lanes might be yields, but shouldn't need to represent that
visually.

- Easy representation: draw red line / stop sign in some driving lanes. Leave the priority lanes alone.
- Harder: draw a stop sign on the side of the road by some lanes. Won't this look weird top-down and at certain angles?

Traffic signals?
- per lane would be weird.
- drawing turn icons as red/yellow/green is pretty clear...
- could draw an unaligned signal box with 3 circles in the middle of the intersection, but what does it represent? maybe just an initial indicator of what's going on; not full detail.
- similarly, draw a single stop sign in the middle of other intersections? :P
