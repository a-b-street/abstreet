# TODO - GUI and UX

## Fix existing stuff

- if a lane could feasibly have multiple turn options but doesnt, print "ONLY"
- audit all panics
- tune text color, size, padding

- click cycle diagram to edit duration

- yellow or flashing red/yellow for yields
- text box entry: highlight char looks like replace mode; draw it btwn chars

## General ezgui stuff

- arbitrary viewports?!
- tiling wm

## New features

- collapse smaller roads/neighborhoods and just show aggregate stats about them (in/out flow, moving/blocked within)

## Better rendering

- depict residential bldg occupany size somehow
- rooftops
	- https://thumbs.dreamstime.com/b/top-view-city-street-asphalt-transport-people-walking-down-sidewalk-intersecting-road-pedestrian-81034411.jpg
	- https://thumbs.dreamstime.com/z/top-view-city-seamless-pattern-streets-roads-houses-cars-68652655.jpg
- general inspiration
	- https://gifer.com/en/2svr
	- https://www.fhwa.dot.gov/publications/research/safety/05078/images/fig6.gif
	- http://gamma.cs.unc.edu/HYBRID_TRAFFIC/images/3d-topdown.jpg
- color tuning
	- neutral (white or offwhite) color and make noncritical info close to
	  that. http://davidjohnstone.net/pages/lch-lab-colour-gradient-picker,
          chroma < 50

## Performance

- it's a pity we have to redo DrawCar work for all those parked cars every tick
- show FPS or some kind of measure of lag
- sleep better in the event loop
	- first make UserInput borrow state and not need to consume
- more speculative performance ideas
	- specialized shaders for common shapes like circles?
	- try https://docs.rs/dymod/0.1.0/dymod/ to link in a release-mode ezgui crate?

## Depicting traffic unzoomed

- strange things to depict
	- cars partly straddling roads
	- some lanes backed up, others moving
	- peds (lots of them in one position maybe!)
	- intersections (simultaneous turns, some blocked, others not)
	- peds waiting for bus
- general ideas
	- darked colors (contrast map bg and road)
	- show min/max bounds (exact max is hard, but could calculate best-case easily)
	- percentage of capacity instead of an exact, moving length
- criteria
	- at low zoom, easily pinpoint where things are moving and stuck
	- include all agents
