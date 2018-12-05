# Tutorial mode

## Synthetic maps

For tests and tutorial mode, I totally need the ability to create little
synthetic maps in a UI. Should be different than the main UI.

What are the 'abstract' objects to manipulate?

- Intersections... just points
	- Move these, have the roads move too
- Ability to connect two intersections with a straight line road
	- Edit lane type list in each direction
	- This lets border nodes be created
- Place rectangular buildings

This should basically use raw_data as primitives... or actually, no. GPS would
be weird to work with, and roads should be expressed as the two intersections,
so we don't have to update coordinates when we move intersections.

How to map lanes to stuff that make/lanes.rs will like? Might actually be easy,
actually.

Ideally, would render the abstract thing in one pane, and live-convert to the
full map and display it with the editor code in the other pane. But as the
halloween experiment shows -- that'd require a fair bit more refactoring first.

## Playing the levels

I would say individual tutorial levels could just be running a scenario, except...

- I don't want to hand-write scenario JSON. Code greatly preferred.
- I might want to pop up some dialogs at the beginning / middle / end to explain stuff
	- and these dialogs don't belong in the main editor crate
- Need to have listeners inspect sim state to detect when the player is correct/wrong
- All of the editor's controls don't make sense in tutorial mode (defining a new scenario)

I almost want to extract most stuff from the editor crate into a reusable
library, then use it from a new tutorial mode crate and the existing editor. But...

- the editor's current plugin architecture doesnt allow for specifying plugin
  interactions, which feels very relevant if these become generic
- some plugins know about primary/secondary state and plugins. do the concepts
  of per UI and per map have to be generic too, or is that just for editor?

- maybe 'modes' are collections of mutually compatible plugins?
- it's weird that plugins always exist as singletons, with explicit inactive states. maybe need to have new(input) -> option<plugin> and make event() indicate when the plugin should be destroyed. then a mode is an invocation of these, with its tiny little list of active plugins (boxed or maybe not)
- and a tutorial level is just one of these modes, maybe with some kind of setup() that clobbers the map and sim.
- think of modes as just event handlers! straight line code. the plugin trait is maybe not so useful. can share stuff like toggleable layerlike things by a SUBROUTINE

### Simple workflows

Think about the tutorial mode as a FSM. For now, no live editing while a sim is running.

- first make the user go through some dialogs
	- still displaying the map in the background, but no interaction yet
- then be in static explore mode
	- can do: canvas movement, debug objects, hide objects, toggle layers, 
	- can do things that are 'active plugin' and eat keyboard, but should pop to previous state
		- display log console, save map edits


a_b_tests.rs       diff_worlds.rs         layers.rs                scenarios.rs      steep.rs
chokepoints.rs     draw_neighborhoods.rs  logs.rs                  search.rs         stop_sign_editor.rs
classification.rs  floodfill.rs           map_edits.rs             show_activity.rs  time_travel.rs
color_picker.rs    follow.rs              mod.rs                   show_owner.rs     traffic_signal_editor.rs
debug_objects.rs   geom_validation.rs     neighborhood_summary.rs  show_route.rs     turn_cycler.rs
diff_all.rs        hider.rs               road_editor.rs           sim_controls.rs   warp.rs
