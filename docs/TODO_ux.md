# TODO - GUI and UX

## Fix existing stuff

- try showing traffic signals by little boxes at the end of lanes
	- red circle means right turn on red OK, red right arrow means nope, green means normal turns ok, green arrow means protected left, crosswalk hand or stick figure

- if a lane could feasibly have multiple turn options but doesnt, print "ONLY"
- color roads as solid black when zoomed out, and make intersections similar (except for stop sign / signal)
- audit all panics
- tune text color, size, padding
- sort the top menus

- click cycle diagram to edit duration

- revamp stop sign editor
- toggle rewind mode
	- sim stuff feels like a modal menu that's kinda omniprescent, but gets hidden sometimes
- yellow or flashing red/yellow for yields
- text box entry: highlight char looks like replace mode; draw it btwn chars

- traffic signal cycles go offscreen sometimes!
- mouseover shouldnt even be possible in lot of modes, like when a menu is active

## General ezgui stuff

- optionally limit canvas scrolling/zooming to some map bounds
- top menu doesnt know when we have a more urgent input thing going!
- cant use G for geom debug mode and contextual polygon debug
- X on all menus
- when dragging, dont give mouse movement to UI elements
- start context menu when left click releases and we're not dragging
- dont draw context menu off-screen
- can we change labels in modal or top menu? show/hide
- stacked modal menus
	- should quit key for modal menus match key that started it?
	- launch floodfill from context menu while following an agent... shouldnt be allowed
	- can coexist: show score, search, hide
	- some abstraction to just declare set_mode, give the extra width and height besides the menu, and get back a screenpt to start drawing at
- bold hotkey letters

## New features

- swap direction of one-way
- convert between one- and two-way if there's enough space
- collapse smaller roads/neighborhoods and just show aggregate stats about them (in/out flow, moving/blocked within)
- undo support for edits

## Better rendering

- depict residential bldg occupany size somehow
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

## Switch to OpenGL (for speed)

- speed
	- show FPS or some kind of measure of lag
	- sleep better in the event loop
		- first make UserInput borrow state and not need to consume
- quality
	- need padding around text
	- text entry needs to draw the cursor differently
- more speculative performance ideas
	- experiment with batching and not passing colors
	- specialized shaders for common shapes like circles?
	- try https://docs.rs/dymod/0.1.0/dymod/ to link in a release-mode ezgui crate?

## Performance

- it's a pity we have to redo DrawCar work for all those parked cars every tick
- areas like lakes are incredibly detailed
