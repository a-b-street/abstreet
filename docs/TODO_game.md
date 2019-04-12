# TODO - Core gameplay

- parking vs bus tutorial level
	- big vertical road. some neighborhoods off to the side. everyone trying to go nrth or south. one driving, one parkign lane. a bus that makes a loop. some horizontal roads connected to borders that force the vertical road to not be fast. just make a dedicated bus lane! oops,except some of the bus stops are at narrow places, so traffic has to slow down anyway for the bus. maybe there's a bypass road through a neighborhood?
- introduce elements gradually... fix a silly traffic signal with just cars, then add peds and bikes...

- Example use cases
	- maybe dont need simulation at all to play with these
		- just a smooth editing and diffing UI
	- montlake/520 turn restrictions with pedestrian scramble
	- close interior neighborhoods to most cars (except for src/dst), see how traffic restricted to arterials would work
		- puzzle: with only X miles of retained road, where would you leave them? what roads would go away?
	- create a bike network with minimal hills, dedicated roads, minimal crossings

- easter eggs
	- name agents, with some good names scattered in (Dustin Carlino, Dustin Bikelino, Dustin Buslino...)

## More things to simulate

- Light rail and downtown bus tunnel
- more accurate pedestrian speed, and randomized speeds
- seed parked cars in neighborhood with no owner or a far-away owner, to model reasonable starting state
- outgoing border nodes can throttle to simulate traffic downstream

## Small bugs / tests to add

- do bikes use bike lanes?
- test that peds will use buses organically
	- make sure that we can jump to a ped on a bus and see the bus
