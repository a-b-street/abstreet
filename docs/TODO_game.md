# TODO - Core gameplay

- Example use cases
	- montlake/520 turn restrictions with pedestrian scramble
		- need intersection merging before this is understandable
	- close interior neighborhoods to most cars (except for src/dst), see how traffic restricted to arterials would work
		- puzzle: with only X miles of retained road, where would you leave them? what roads would go away?
		- parking garages on the edges of neighborhoods?
	- create a bike network with minimal hills, dedicated roads, minimal crossings

- charm
	- name agents, with some good names scattered in (Dustin Carlino, Dustin Bikelino, Dustin Buslino...)
	- music / sound effects
		- as you zoom in, overhear conversations and such
	- some buildings could have extra detail
	- zoom in too much, what might you see? ;)
	- loading screen: snakey cars
	- game intro/backstory: history of seattle urban planning
	- player context: a drone. people will look up eventually.

## Tutorial

- parking vs bus tutorial level
	- big vertical road. some neighborhoods off to the side. everyone trying to go nrth or south. one driving, one parkign lane. a bus that makes a loop. some horizontal roads connected to borders that force the vertical road to not be fast. just make a dedicated bus lane! oops,except some of the bus stops are at narrow places, so traffic has to slow down anyway for the bus. maybe there's a bypass road through a neighborhood?
- introduce elements gradually... fix a silly traffic signal with just cars, then add peds and bikes...

- spawn cars somewhere
- run sim, pause, change speed, reset

## More things to simulate

- Light rail and downtown bus tunnel
- seed parked cars in neighborhood with no owner or a far-away owner, to model reasonable starting state
- outgoing border nodes can throttle to simulate traffic downstream
