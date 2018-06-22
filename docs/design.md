# Design notes

## Associated data / ECS

So far, the different structures for representing everything associated with a
road/intersection/etc strongly resembles ECS. Would explicitly using an ECS
library help?

http://www.gameprogrammingpatterns.com/component.html

Road has different representations:

- protobuf
- runtime map_model (mostly focusing on the graph)
- UI wrapper + geometry for simulation (should probably tease this apart)
- "control" layer for editable policies
- Queue of cars on the road

It could be useful to bundle together a context-like object of Map, GeomMap,
ControlMap, DrawMap, etc.

Need to figure out how to handle reaping old IDs for transient objects like
cars, but also things like modified roads. Slot maps?

## Immediate mode GUI

Things organically wound up implementing this pattern. ui.rs is meant to just
be the glue between all the plugins and things, but color logic particularly is
leaking badly into there right now.

## Strawman driving model

- Show the FSM
- Explain how the model is based on best-case bounds
- Position is derived lazily from time
- How accurate could it be? Based on inner-city speeds and timesteps

- problems
	- straw model has some quirks with queueing
		- after the lead vehicle starts the turn, the queue behind it magically warps to the front of the road
		- the first vehicle in the turn jumps to a strange position based on the front/back rendering
	- at signals, cars doing the same turn wont start it until the last car finishes it

## Stop sign editor

Stop signs are FIFO, except that many intersections only have a stop sign for
some sides. Going straight on the priority roads is immedite, and left turns
from those priority roads also take precedence over the low-priority roads. So
should the stop sign controller mark individual turns as priority/not, or
individual roads, with implied semantics for left turns? There are really 3
priorities if turns are considered...

Figuring out nonconflicting roads seems tricky. For now, going to have a
complicated UI and let individual turns be classified into 3 priority classes.
First group can't conflict, second and third groups can conflict and are FIFO.
Will probably have to revisit this later.

## Sim state equality and f64's

Currently using si::Second<f64> for time, which means comparing sim state by
deriving Eq is a headache. Since the timestep size is fixed anyway, this should
just become ticks. Was tempted to use usize, but arch dependence is weird, and
with a 0.1s timestep, 2^32 - 1 ticks is about 13.5 years, which is quite a long
timescale for a traffic simulation. :) So, let's switch to u32.

## UI plugins

- Things like steepness visualizer used to just be UI-less logic, making it
  easy to understand and test. Maybe the sim_ctrl pattern is nicer? A light
adapter to control the thing from the UI? ezgui's textbox and menu are similar
-- no rendering, some input handling.
