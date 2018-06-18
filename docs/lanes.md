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
