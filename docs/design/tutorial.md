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



Gah, bite this off in slow pieces. First find layer-like things... things that
draw/hide stuff. Tend to have very simple activate/deactive controls. No sim interaction.

Or maybe with the exclusive editors...
- in the short term, could make a plugin that just delegates to a smaller list
	- except some are per-map and some are not
		- wouldnt need that concept if we dont store plugins when
		  theyre inactive and have a notion of what can simultaneously
		  be active...
- figure out what other plugins are valid alongside the exclusive editors...
	- should activating an editor reset toggleable layers and hidden stuff? that's debug state...
	- when is moving via mouse still valid? color picker (right?), neighborhood, road/intersection editors
	- intersection and road editors... just debug. and actually, is that even true?
	- running a sim / doing time travel shouldnt be valid


debug stuff: toggleable layers, hider, geom validation, floodfill

alright maybe we actually do have lots of exclusive states...
- the exclusive editors
	- note that if we're running an A/B test, none of these are valid! cant edit stuff during a/b test... just run.
- explore
	- sim controls | time travel
	- bunch of nonblocking stuff... chokepoints, classification, debug, diff all, diff trip, floodfill, follow...
		- different keys to deactivate them? :P
	- some blocking stuff... search, warp. when these're around, run them first


- dir structure... all the exclusive stuff side-by-side, and then a shared/ for stuff that might apply during different states
- the exclusive editors: a_b_tests.rs     draw_neighborhoods.rs  map_edits.rs    scenarios.rs         traffic_signal_editor.rs
color_picker.rs  road_editor.rs  stop_sign_editor.rs




maybe as an initial step, can we get rid of plugins per map vs per UI and just have one or the other?

- every plugin per map?
	- toggleable layers then arent shared... fine
	- logs is per UI? whoa, that's actually maybe messy!
	- sim ctrl, diff world/trip does need to be independent though.
- every plugin per UI?
	- when we load a new map from edits, still have to replace the world.
	- would have to argue that no plugin that keeps per-map state can run during A/B test mode!
		- or rather, that we cant swap while any of those plugins hold state!
		- show owner, turn cycler, debug, follow (need to recalculate)
		- time travel (needs a/b support generally)
		- show route (totally valid to swap while this is going... grrr.)



maybe step 1...
- make a single 'Mode' for exclusive editors
	- skip it if a secondary sim is present (aka in A/B mode)
	- it lives per UI, because of above condition
	- for now, impl it as a hierarchial plugin itself that just delegates
	- keep plugin trait for each of em for convenience in edit mode, though.
	- each of the editors can stop having inactive state. have new() that returns option

and probably step 2...
- start smaller, a Debug mode... stuff that shouldnt really be relevant in tutorial mode, for example
	- chokepoints, classification, floodfill, geom validation, hider, toggleable layers, steep
	- arguably some of these could stack, but I don't care much yet... dont worry about ambient plugins yet

	- each of the editors can stop having inactive state. have new() that returns option
	- the permanent ones (hider and toggleable layers) shouldnt even implement Plugin; theyre custom weirdness
- make a single 'Mode' for normal exploration
	- the blocking ones: warp
	- the ambient ones: debug objects, follow, neighborhood summary, show activity, show owner, show route, turn cycler
		- still represent the inactive state? for now, sure
		- have to solve the problem of overlapping keys to quit
	- what is search? should it be ambient or not?
	- dont forget neighborhood summary


	- this has to be completely per UI or completely per map
	- let a bunch of plugins run non-exclusively there, as relevant
		- AmbientPlugin trait, maybe? or maybe just explicitly call on each field in order
	- and still have a single blocking plugin possible, like warp
	- rewrite turn_cycler; i dont understand it. also it used to block input after starting to tab through stuff. weird?

	thursday pick-up:
	- search (sometimes ambient, sometimes blocking)
	- warp (blocking)
	- overlapping keys to quit stuff...

and step 3...
- dismantle the plugin abstraction in UI and probably also the trait. do something different for modes.
- clean up event vs new_event
- use Escape to quit most plugins, since it'll only be callable normally from some modes
- make it more clear that keys cant overlap... in each mode, specify the trigger key it uses?
	- except some of them are more conditional and that makes overlap fine
- can we get rid of PluginsPerUI almost? since we'll likely get rid of plugins entirely... yeah?
	- view and debug mode can coexist!
