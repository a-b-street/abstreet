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
	- FIRST: move geom things into the Map structs directly. get rid of that crate.
		---> option 1: module per object type, geometry and graph squished together
		- option 2: try to separate the graph/geom stuff within map model.
	- CLEANUP: no more 'extern crate' in non lib
	- CLEANUP: gps to screen in map upfront, dont plumb along gps pts for bldg/parcel/etc, so bounds should become private.
		- pt2d should no longer represent gps
	- CLEANUP: bldg front path should happen upfront, not in render


	- THEN: express the proto -> runtime map loading as a sequence of phases
		- keep doing the current road trimming for the moment
		- later, this could be the same as the OSM conversion. just
		  like aorta's map make. but instead, be able to restart from
		  any point, by the magic of easy serialization.
		- get rid of the protobuf

	- line trimming
		- just replacing the last pt might not always work. especially with old center lines!


- an mvp release could just be producing high-quality, reusable geometry for seattle
	- with an editor to quickly fiddle with where sidewalks/different lanes are
