# Walking-related design notes

## Crosswalks

- Turns go from a src to a dst, so we'd need to double them for crosswalks, since they're always bidirectional
- Turn icons might not make sense as a UI?
- Many sidewalks directly connect at corners and shouldn't have anything drawn for them
- We don't want to draw diagonals... just from one side of the road to the other
- We want crosswalks at the beginning AND end of the sidewalk!

- v1: remember other_side for sidewalks too. draw crosswalks at the beginning AND end of every sidewalk lane.
- do extra drawing in DrawIntersection for now, figure out modeling later.

- alright, directional lanes and turns dont fit sidewalks at all. turn icons
  are drawn at one end. the turns-in-an-intersection invariant is broken, since
  src and dst dont match up for one side.
- could kind of cheat by doubling lanes for sidewalks and making the geometry
  overlap, but this feels like a worse hack. it's very tempting to ditch lanes
  and turns for a different way to model sidewalks and crosswalks.
	- for now, let's make a distinct road and lane abstraction and plumb that all the way through. see what it'd be like to have some more primitives:

- Intersection
- Building
- Parcel
- Road (undirected bundle)
- Driving/biking Lane (directed)
- Sidewalk (undirected)
- Parking lane (no orientation... or, kind of like a driving lane)
- Turn (directed and not)

but the fact that sidewalks are oriented is actually convenient, it makes it clear that incoming's last pt should be glued to outgoing's first pt.

what if we just add a bit and make turns bidirectional? still express them in the directional way?
if we're looking at turns from a road that's a sidewalk, bake in some extra logic?

## Pedestrian modeling

- Is it useful to distinguish CarID and PedestrianID? What about when an agent has a multi-modal trip? Probably become AgentID later.

- Worth mentioning that I'm assuming pedestrians don't queue or collide. In
  most reasonable sidewalk cases, this is true. Don't need to model more
  detailed movement. As a consequence of this, crosswalk turns never conflict.
  Assume people can weave.

## Cost of contraflow

Either duplicate sidewalks in both directions (extra rendering and memory, etc)
or have complicated turns and contraflow logic. Making trace_route is example
of time wasted on contraflow mess. Maybe having two directional sides of a
sidewalk is nice anyway? What about the funky turns causing a ped to not even
cross a sidewalk at all and immediately chain together two turns?

Small complication with two directional sidewalks
	- SidewalkSpots get more complicated. are they associated with the
	  original direction always? How to start/end walking?
	- do they belong to children {forwards, backwards}? They'd no longer be
	  in order.

And what about modeling shared left-turn lanes?
