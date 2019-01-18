# TODO - Core gameplay

- parking vs bus tutorial level



- Example use cases
	- montlake/520 turn restrictions with pedestrian scramble
	- close interior neighborhoods to most cars (except for src/dst), see how traffic restricted to arterials would work
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
