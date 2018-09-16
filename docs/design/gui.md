# GUI-related design notes

## Moving away from piston

- options
	- gfx (pre-II or not?) + winit
		- https://suhr.github.io/gsgt/
	- glium (unmaintained)
	- ggez: https://github.com/nical/lyon/blob/master/examples/gfx_basic/src/main.rs
		- too much other stuff

https://www.reddit.com/r/rust_gamedev/comments/7f7w60/auditioning_a_replacement_for_bearlibterminal/

https://github.com/ggez/ggez/blob/master/examples/drawing.rs

things to follow:
	- https://suhr.github.io/gsgt/
	- https://wiki.alopex.li/LearningGfx
	- https://github.com/nical/lyon/blob/master/examples/gfx_basic/src/main.rs

	- porting to wasm: https://aimlesslygoingforward.com/blog/2017/12/25/dose-response-ported-to-webassembly/

## Refactoring UI

I want to make bus stops be a selectable layer, and I want to add lake/park
areas soon. What did
https://github.com/dabreegster/abstreet/commit/b55e0ae263fcfe4621765a7d4a8d208ab5b89e76#diff-8257d0ba4a304de185c0125ff99e353b
have to add?

- color scheme entry
- an ID (displayable)
= boilerplate in selection plugin
	= a way to ask for tooltip lines
- boilerplate in the hider plugin
- DrawExtraShape
	- draw, contains_pt, get_bbox, tooltip_lines
- DrawMap
	- list of stuff, with ID lookup
	- a quadtree and way to get onscreen stuff
- UI
	- a toggleablelayer for it
		= and clearing selection state maybe
	- are we mouseover it? (right order)
	- draw it (right order)
	- pick the color for it

try a quadtree with any type of object.


alright, time to move color logic. let's see what it takes for each Renderable to decide its own color -- what state do they need?
- dont forget: search active and lane not a match does yield a color.
- woops, sim Draw{Car,Ped} doesnt implement Renderable. but it'll be nested anyway... we dont want to move stuff in the quadtree constantly. the 'get everything onscreen' routine should do that, interpreting Lanes and Intersections in initial pass of results correctly by querying for more stuff.
- actually, still undecided about color and RenderOptions...
	- the whole motivation -- one draw() interface can only return a single color. and dont want to add code to UI for every single object type.
		- maybe send in Option<Box<Plugin>> of the current thing, and then it can have a more specific method for each object type? or is that too far the other direction again?
	- send in Option<Color>, letting each plugin vote?
	- maybe if we only have one or two active plugins, it'll be easier to understand?
	- would it be weird to invert and send in all the plugins? the point is
	  kinda for there to be one place to handle interactions between
          plugins -- UI. having a strong concept of one active at a time would probably
          _really_ help.
	- maybe plugins do have a single color_obj(enum of IDs) -> Option<Color>?
- make it easier to fill out RenderOptions for things generically
- next step: extra things like draw_front_path also take cs, not a specific color -- if it makes sense.


- refactor selection plugin's color_something
- selection plugin currently has this weird case where it can cycle through turns at an intersection. MOVE THAT out.
	- more generally, move out all custom logic. make other things know if something is selected and do special stuff if so.
	- and make it act generic at last! \o/


OK, so where was I?
- colors are still currently missing for things that need two of them.
** having one active plugin at a time simplifies the color problem and solves a few others, so try that now.
	- another refactor to do -- initiate plugins based on current_selection_state in UI or the plugin, but stop mixing so much
- make car and ped also Renderable, for great consistency!
- work on generic quadtree idea

### One active plugin at a time

I wonder if the UI will have a need to reach into plugins beyond event(). Let's find out!
- exceptions
	- hider needs to given to finding onscreen stuff
	- search can specify colors and OSD lines
	- warp can add OSD lines
	- show_route can color
	- floodfiller can color
	- steepness can color
	- osm can color
	- signal and stop sign editor can color and indicate what icons are onscreen
	- road editor can be asked for state to serialize
	- sim ctrl can contribute OSD lines (and everything grabs sim from it directly too -- maybe sim should live in UI directly)
	- color picker can draw
	- turn cycler can draw

- the stuff they take in event() is different. hmm.
	- box lil closures

so it feels like we implicitly have a big enum of active plugin, with each of their states kinda hidden inside.

- the simple idea
	- UI keeps having a bunch of separate plugins with their real type
	- have a list of closures that take UI and do event(). return true if that plugin is active
		NOT if something was done with the input
	- in event(), go through the list and stop when something becomes
	  active. remember it's active and just call it directly next time in
          event(), until it says its no longer active.
	- then figure out the implications for color

	tomorrow:
	= implement exclusive active plugin thing
	= rethink if existing plugins are active or not. maybe dont make event() return if active or not, maybe have a separate method? or just in event, do stuff first, and then have this query at the end.
	= tooltips shouldnt be shown while a random plugin is active; move that out to its own plugin! same for debugging. selection plugin should have NO logic.
	= refactor the toggleablelayer stuff, then move them to list of plugins too
	= clean up selection state... should warp and hider be able to modify it, or just rerun mouseover_something?

	= initiate plugins in the plugin's event; stop doing stuff directly in UI
	= basically, make UI.event() just the active plugin list thing as much as possible.
	- deal with overlapping keys that still kinda happen (sim ctrl, escape game)
	- bug: do need to recalculate current_selection whenever anything potentially changes camera, like follow

	= make draw car/ped implement renderable.
		= move Draw* to editor crate, and have an intermediate thing exposed from sim and do the translation in get_blah_onscreen.
	
	- then rethink colors, with simplified single plugin
	= then finally try out a unified quadtree!
		= make parcels selectable as well?

	- and see how much boilerplate a new type would need, by adding bus stops and water/parks


Alright, replan yet again.

= then rethink colors, with simplified single plugin
	= plugin trait, color(id) -> Option<Color>. parallel list of box plugins (or, a fxn that takes the idx)
	= refactor to one color_blah method
	= handle the two color things... just buildings?
= and see how much boilerplate a new type would need, by adding bus stops and water/parks

- load water/parks and stuff
- deal with overlapping keys that still kinda happen (sim ctrl, escape game)
	- and missing keys, like no tooltip for turns, since they're only shown in editors now
- bug: do need to recalculate current_selection whenever anything potentially changes camera, like follow
- consider merging control map into map
- see how hard it is to render textures onto cars or something
= refactor debug and tooltip lines for objects

## Immediate mode GUI

Things organically wound up implementing this pattern. ui.rs is meant to just
be the glue between all the plugins and things, but color logic particularly is
leaking badly into there right now.

## UI plugins

- Things like steepness visualizer used to just be UI-less logic, making it
  easy to understand and test. Maybe the sim_ctrl pattern is nicer? A light
adapter to control the thing from the UI? ezgui's textbox and menu are similar
-- no rendering, some input handling.

## GUI refactoring thoughts

- GfxCtx members should be private. make methods for drawing rectangles and such
	- should be useful short term. dunno how this will look later with gfx-rs, but dedupes code in the meantime.
- should GfxCtx own Canvas or vice versa?
	- Canvas has persistent state, GfxCtx is ephemeral every draw cycle
	- dont want to draw outside of render, but may want to readjust camera
	- compromise is maybe storing the last known window size in canvas, so we dont have to keep plumbing it between frames anyway.


One UI plugin at a time:
- What can plugins do?
	- (rarely) contribute OSD lines (in some order)
	- (rarely) do custom drawing (in some order)
	- event handling
		- mutate themselves or consume+return?
		- indicate if the plugin was active and did stuff?
- just quit after handling each plugin? and do panning / some selection stuff earlier
- alright, atfer the current cleanup with short-circuiting... express as a more abstract monadish thing? or since there are side effects sometimes and inconsistent arguments and such, maybe not?
	- consistently mutate a plugin or return a copy
	- the Optionals might be annoying.
