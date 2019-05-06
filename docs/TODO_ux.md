# TODO - GUI and UX

## Fix existing stuff

- try showing traffic signals by little boxes at the end of lanes
	- red circle means right turn on red OK, red right arrow means nope, green means normal turns ok, green arrow means protected left, crosswalk hand or stick figure

- if a lane could feasibly have multiple turn options but doesnt, print "ONLY"
- color roads as solid black when zoomed out, and make intersections similar (except for stop sign / signal)
- audit all panics
- tune text color, size, padding

- click cycle diagram to edit duration

- revamp stop sign editor
- toggle rewind mode
- yellow or flashing red/yellow for yields
- text box entry: highlight char looks like replace mode; draw it btwn chars

- traffic signal cycles go offscreen sometimes!

- navigator
	- show options on map
	- stop jumping text size

## General ezgui stuff

- optionally limit canvas scrolling/zooming to some map bounds
- when dragging, dont give mouse movement to UI elements
- start context menu when left click releases and we're not dragging
- can we change labels in modal or top menu? show/hide
- label sections of modal menus
- distinguish hints from status of modal menus, for hiding purposes
- move context menus out of ezgui
	- simplify/remove UserInput.
	- maybe separate impls for context, wizard, modal menu make sense.

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

## Mission Edit Mode

- neighborhood
	- how to "pop" up the menu, go back to load/create and even to managing scenarios? feels like a stack.
	- display some instructions in the modal thing ("move a point by grabbing it")
	- warp to neighborhood center and zoom out when loading one (or even hovering in the menu?)
	- display all and click to edit
		- draw text in map-space, or be able to scale it
	- renaming
- scenario
	- visualize should just be the default thing
	- summarize in the modal menu, dont display the ugly text
	- almost feels like a list of 3 'command' types, each of which can be visualized:
		- seed cars
		- spawn agents
		- spawn agents from border
	- kind of need to CRUD this list
	- time input is very unclear, put help text in there
	- combine the spawn and border spawn... choose neighborhood OR border
		- choose in a menu list, or click on the map
			- draw the border nodes loudly
	- visualize better
		- draw separate arrows src/dst, on each side of text
			- draw text at a fixed size, not in screenspace, specify the font size.
			- associate font size with Text, probably
		- highlight a region, draw counts to/from it in some meaningful way
		- timer slider (except timeslices arent neatly in hour blocks, though they maybe should be)
		- a table (with color-coded entries) is actually perfect
