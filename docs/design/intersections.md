# Intersection-related design notes

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

## Stop signs

How to depict stop signs? Each driving lane has a priority... asap go or full
stop. Turns from go lanes might be yields, but shouldn't need to represent that
visually.

- Easy representation: draw red line / stop sign in some driving lanes. Leave the priority lanes alone.
- Harder: draw a stop sign on the side of the road by some lanes. Won't this look weird top-down and at certain angles?

## Traffic signals

- per lane would be weird.
- drawing turn icons as red/yellow/green is pretty clear...
- could draw an unaligned signal box with 3 circles in the middle of the intersection, but what does it represent? maybe just an initial indicator of what's going on; not full detail.
- similarly, draw a single stop sign in the middle of other intersections? :P

## Intersection policies for pedestrians ##

Before figuring out how pedestrians will deterministically use intersections alongside cars, recall how cars currently work...

- ask all cars for next move (continue on same thing, or move to a turn/lane)
- using fixed state, adjust some of the moves that dont have room to move to a new spot to wait instead
- serially ask intersections if a car can start a turn
- serially make sure only one new car enters a lane in the tick
	- shouldnt the intersection policy guarantee this by itself?
- very awkwardly reset all queues from scratch

How did AORTA do it?

- agent.step for all of em (mutate stuff)
	- enter intersections, telling them. must've previously gotten a ticket
- let all the agents react to the new world (immutable, except for IDEMPOTENTLY asking for turn)
	- here we ask for tickets, unless we've already got one
- same for intersections
	- grant them here

aka basically yeah, the simple:

- agents send a ticket during the planning phase?
- intersections get a chance to react every tick, granting tickets
- during the next action phase, an agent can act on the approved ticket?

good pattern in intersections:
- a sim state that the rest of the code interacts with for ALL intersections. rest of code doesnt see individual objects.
- that manager object delegates out most of the logic to SPECIALIZED versions of individual objects and does the matching
	- no need for this to exist on the individual IntersectionPolicy object

How to share common state in intersections?
- if it's just the accepted set, have a parallel array and pass it into step()
	- data locality gets ruined, this is ECS style, bleh
- have a common struct that both enum variants contain
	- still have to match on enum type to operate on it commonly!
- have one struct that then contains an enum
	- when delegating to specialized thing, can pass this unpacked thing down, right?

Seeing lots of deadlock bugs from accepting non-leader vehicles. For now,
switch to only considering leader vehicles, and later maybe relax to anybody
following only accepted vehicles.

Leader vehicle is a bit vague; could be leader on current queue, which is still a bit far away.

## Stop sign priority

Use OSM highway tags to rank. For all the turns on the higher priority road, detect priority/yield based on turn angle, I guess.
