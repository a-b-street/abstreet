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
		- would need to be able to enter/exit a sidewalkspot from
		  either directional lane. modeling 'left turns' into/out of
		  sidewalk spots is way overkill.
	- do they belong to children {forwards, backwards}? They'd no longer be
	  in order.
	- overlapping geometry is wasteful and makes debugging confusing
		- could have two distinct sides of the sidewalk

And what about modeling shared left-turn lanes?
	- Are these even that important to model? Usually used for turning into
	  parking lots or driveways, which we're not modeling at all.

One-way sidewalk lanes would NOT solve the turn-chains:
- think about crossing N, then W at a 4-way. legitimately doing two turns in sequence. and this is fine!
	- executing two turns in sequence might be tricky

An alternative:
- in sim, pathfinding, map model trace, etc layers only, using some new
  abstraction instead of raw lanes and implied turns
	- big point here: why dont pathfinding routes explicitly list turns?
	  then it's clear when a ped doesn't cross a lane and just does two
	  turns in sequence
	- the code to choose turns is kind of annoyingly repeated in some
	  places anyway
	- this probably makes lookahead-type behavior simpler
	- this abstraction can just say whether to go backwards on a sidewalk or not
	- whether or not sidewalks later get split into 2 lanes, I think this
	  would be helpful.
- places to change...
	- map model pathfinding.
		- proper type, backed by VecDeque
		- backrefs can store the intermediate piece often
		- complication with not crossing a sidewalk? maybe that can be
		  deduped there, in one spot
	- trace_route should move to become part of this Path type
		- no more partly duped code btwn walking/driving
		- Traversable::slice can probably also go away, or only get
		  called by this one place?
	- sim layer no longer needs to pick turns
	- walking code no longer needs to calculate contraflow itself!
		- maybe should plumb start/end dist_along into pathfinding too?

## Crosswalks again, after making peace with contraflow

Two types of things to render

- actual crosswalks
- shared corners

What're the desired turns?

- the crosswalks shouldn't go to the other_side; they should be N<->S
- the shared corners are... obvious
- both of these should be bidirectional

Don't duplicate work with DrawIntersection... let's annotate turn types.
- Crosswalk
- SharedSidewalkCorner
	- these 2 exclusively mean sidewalks
- can classify straight/right/left turns too, once. so control layer gets a break.

actually, keep the turn angle stuff independent of sidewalk stuff.
