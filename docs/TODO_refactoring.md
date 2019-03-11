# TODO - Refactoring

- easier way to define magic tuneable constants
	- and maybe to recalculate fixedish things if they change?

## Map layer

- fixed precision math
	- more careful geom types, with negative/positive cases
	- also bounds?
	- cant get rid of the ccw intersection check... different answer in some cases that looks bad

- maybe also the time to split into different lane types? what's similar/not between them?
	- graph querying?
	- rendering (and other UI/editor interactions)?
	- sim state?
	- Sidewalk, Parking, Street

- make synthetic use raw stuff directly?
	- lonlat vs pt is annoying; have to use bounds to balloon to world at least once

## Sim layer

- rename Car->Vehicle?

## ezgui layer

- probably use f32, not f64 everywhere... but after Pt2D becomes fixed size
- undo the y inversion hacks at last!
- ezgui passes EventCtx and DrawCtx with appropriate things exposed.
	- maybe move glyph ownership out of canvas entirely. dont need RefCell.
		- need to pass around a NonDrawCtx very uniformly first for this to work
	- canvas owning text-drawing is maybe a bit weird, at least API-wise
	- hide stuff inside the ctx's? canvas and prerender shouldnt even be known outside of crate
- generic World with quadtree should have actions on objects

## Editor layer

- plugin APIs are weird
	- ambient_event and one event() indicating done or not. dont express blockingness in that API.
	- actually, take away Plugin trait entirely? Except for the stuff that gets all boxed up?
	- one API for all modes/plugins doesn't make sense maybe. does primary_plugins need to be in PluginCtx at all?
	- can we somehow fold PluginsPerMap into PerMapUI? :D different API that doesnt blindly pass in all of primary field
		- yes, we just have to change the API of everything in there to take a different PluginCtx that doesnt hand over entire primary.

- Layers could be stackable modal too, but do that later. low-pri.

- RenderOptions shouldnt need cam_zoom and debug_mode
- make sure keys from any mode dont overlap. activation keys in one place?
	- stop following + floodfill from lane...
- some plugin state is a bit weird when loading savestates

- rewrite input to understand the 3: context menu, top menu action, or modal things
	- misc keys?
- eventually input won't need to check for dupe keys
	- escape is used in two places now... but who cares?

- consider the decentralized->monolithic redesign
	- could do callbacks that take PluginCtx and that particular plugin. but it becomes awkward to express an action is invalid. write nice imperative code, use control flow.
	- decentralized, stuff like showing original roads is tedious. lots of stackable stuff that can be removed.
