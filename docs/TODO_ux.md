# TODO - GUI and UX

## Performance

- cache draw stuff
	- use a Drawable in DrawPed/DrawCar/DrawBike.
	- fix the process_objects callback nonsense

## Quick n easy

- try showing traffic signals by little boxes at the end of lanes
	- red circle means right turn on red OK, red right arrow means nope, green means normal turns ok, green arrow means protected left, crosswalk hand or stick figure
	- Circle::new radius and pt project_away should take meters type

- if a lane could feasibly have multiple turn options but doesnt, print "ONLY"
- color roads as solid black when zoomed out, and make intersections similar (except for stop sign / signal)
- audit all panics
- tune text color, size, padding
- sort the top menus

- click cycle diagram to edit duration
- lane edit validity
- make it easy to see current lane when changing it

## Less easy

- revamp stop sign editor
- toggle rewind mode
	- sim stuff feels like a modal menu that's kinda omniprescent, but gets hidden sometimes
- yellow or flashing red/yellow for yields
- text box entry: highlight char looks like replace mode; draw it btwn chars

## General ezgui stuff

- trigger screencap from a top menu debug thing WITHOUT a hotkey.
- optionally limit canvas scrolling/zooming to some map bounds
- T top menu doesnt know when we have a more urgent input thing going!
- cant use G for geom debug mode and contextual polygon debug
- on a menu with preselected thing, clicking ANYWHERE does stuff...
- X on all menus
- when dragging, dont give mouse movement to UI elements

## New features

- swap direction of one-way
- convert between one- and two-way if there's enough space

- undo support for edits

## Better rendering

- render overlapping peds reasonably
- draw moving / blocked colors (gradually more red as they wait longer)
- render cars with textures?
- rooftops
	- https://thumbs.dreamstime.com/b/top-view-city-street-asphalt-transport-people-walking-down-sidewalk-intersecting-road-pedestrian-81034411.jpg
	- https://thumbs.dreamstime.com/z/top-view-city-seamless-pattern-streets-roads-houses-cars-68652655.jpg
- general inspiration
	- https://gifer.com/en/2svr
	- https://www.fhwa.dot.gov/publications/research/safety/05078/images/fig6.gif
- color tuning
	- neutral (white or offwhite) color and make noncritical info close to
	  that. http://davidjohnstone.net/pages/lch-lab-colour-gradient-picker,
          chroma < 50
- use new arrows for drawing...
	- triangle and base have a gap; why?!
		- only for turn icons?
		- draw as one polygon when fixed
	- dashed thickness is way off
	- last dash shouldnt appear?

## Switch to OpenGL (for speed)

- speed
	- show FPS or some kind of measure of lag
	- drawing cars is expensive
		- at least cache them until tick changes
	- sleep better in the event loop
		- first make UserInput borrow state and not need to consume
	- optimize zoomed-out experience... one polygon per road
		- make sure the quadtree isn't the limiting factor though
- quality
	- need padding around text
	- text entry needs to draw the cursor differently
	- better arrows (then debug the legend plugin)
	- time for a car texture?
- refactoring
	- probably use f32, not f64 everywhere... but after Pt2D becomes fixed size
	- undo the y inversion hacks at last!
	- ezgui passes EventCtx and DrawCtx with appropriate things exposed.
		- maybe move glyph ownership out of canvas entirely. dont need RefCell.
			- need to pass around a NonDrawCtx very uniformly first for this to work
		- canvas owning text-drawing is maybe a bit weird, at least API-wise
		- hide stuff inside the ctx's? canvas and prerender shouldnt even be known outside of crate
		- generic World with quadtree should have actions on objects
- more speculative performance ideas
	- experiment with batching and not passing colors
	- specialized shaders for common shapes like circles?
	- try https://docs.rs/dymod/0.1.0/dymod/ to link in a release-mode ezgui crate?
