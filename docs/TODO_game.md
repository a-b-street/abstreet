# TODO - Core gameplay

- parking vs bus tutorial level



- Example use cases
	- maybe dont need simulation at all to play with these
		- just a smooth editing and diffing UI
	- montlake/520 turn restrictions with pedestrian scramble
	- close interior neighborhoods to most cars (except for src/dst), see how traffic restricted to arterials would work
		- puzzle: with only X miles of retained road, where would you leave them? what roads would go away?
	- create a bike network with minimal hills, dedicated roads, minimal crossings

- easter eggs
	- name agents, with some good names scattered in (Dustin Carlino, Dustin Bikelino, Dustin Buslino...)


## The very detailed driving model

- different lookahead reaction times
- could see if we ever have a lookahead constraint to deaccel more than what
  we're capable of. it might mask problems. but since things like
  accel_to_stop_in_dist don't have a careful notion of how much time will pass,
  they recommend big rates sometimes.
- no way for an agent to request a turn and ASAP have it granted. are there cases where they might slow down unnecessarily?

- lane changing: seattle polite. planned things work. limit to space needed.

## A much simpler model

- render maps as super simple transit network.

- event based + FSM + radically simpler model. essence of scarcity -- intersections, parking, lane capacity.
- simpler kinematics: if we recognize we're stopping for an
  intersection or following a vehicle closely, just set the
  final state to be perfectly what we desire, rather than
  solving for the acceleration to achieve that.
  intent (and capabilities), not mechanics!
- check out https://github.com/movsim/traffic-simulation-de

- collapse smaller roads/neighborhoods and just simulate aggregate stats about them

## Discrete-event model

- make space toggle play for time travel or sim or simple model
- Make cars cover an entire lane when it's short or long
- avoid impossible accel/deaccel
- Handle lead car being faster
- The speed stated in the middle of intervals is clearly wrong for the follower car

- Prototype alternate driving model
	- branch or forked code?
	- keep timesteps or immediately cut over to events?
	- figure out how lookahead/interruptions and replanning work
