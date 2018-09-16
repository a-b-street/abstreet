# Transit-related design notes

## Bus

Before even looking at the GTFS schema, how should we model buses? They're
basically a special car that just goes from bus stop to bus stop (a
SidewalkSpot) and then just stops for a little while. The route is fixed, but
for now, since pathfinding ignores live traffic, probably fine to ignore this.

- step 1: hardcode two BusStops and hardcode spawn a car that cycles between the two
	- render a bus stop on the sidewalk
		- this actually belongs to the map layer! associated with a sidewalk I guess.
	- render the bus in a special color, and also, make it really long (adjust following dist, but not parking spot len)
	- how to unit test that a bus has reached a stop and is waiting? how do we even know that a bus is at a stop for peds to soon board it? I think a transit state will happen soon...

- step 2: make some peds pick a SINGLE bus to use for their route, if it helps

- step 3: make peds load on the bus and get off at the correct stop. make buses usually wait a fixed time at each stop, but wait a littl extra if loading passengers takes a while.
	- should walking state own peds waiting for a bus?
		- yes: easier drawing, later need to know how crowded a sidewalk is, it's just weird to keep indicating we're at a place. router for cars does this, and the transit sim holds the higher-level state. do the same for now.
			- kind of want walking sim to have a list of peds idling at bus stops. transit sim can let all of them know when a bus arrives!
		- no: transit sim can also contribute DrawPeds. the walking layer has nothing left to do with them... right?

		so:
		1) ped reaches bus stop, writes event. walking sim flips a bit so they stop trying to step(). also has a multimap of bus stop -> waiting peds. they continue to exist on their sidewalk for rendering / crowding purposes.
		2) transit sim gets a step(). for every bus that's waiting, it queries walking sim to see what peds are there. ??? trip thingy will decide if the ped takes the bus or not, but the ownership transfer of ped from walking to transit happens then.
		3) when a bus initially arrives at a stop, it queries all passengers to see who wants to get off and join the walking sim again. the trip thingy decides that.

- step N: load in GTFS for seattle to get real routes and stops

later: multiple transfers, dedicated bus lanes, light rail...

Actually, jump to step 3 and just hardcode a ped to use a route, for now. what should the setup be? hardcode what stop to go to, what route to use, what stop to get off at? trip plan is a sequence...

- walk to a sidewalk POI (bldg, parking spot, bus stop)
- drive somewhere and park
- ride bus route until some stop

for now, these trip sequences can be hardcoded, and planned later.

What's the point of the spawner? It does a few things, and that feels messy:
- vaguely specify a scenario later, with things happening over time.
	- except this is unimplemented, and is probably easier to understand as a list of trips with start times
- a way to retry parked->driving car, since it might not have room
- a way to parallelize pathfinding for the ticks that happen to have lots of things spawning
- a way to initially introduce stuff
	- asap things like a bus route and parked cars
	- indirect orders, like make some parked car start driving creates a trip to satisfy that
- handling transitions to start the next leg of a trip
	- this is the part I want to split out! it's very separate from the rest.


step 1: move existing trip stuff to its own spot, but owned by spawner still
step 2: move interactive and testish spawning stuff to init() or similar, leaving spawner as just mechanics and transitions
	- spawner shouldnt have rng, right?
	- sim needs to hand out its internals (spawner, each model) for the spawning
		- separate methods that take sim and call a special method to get direct access to things?
		- i just physically want the code in a separate file. can we implement a trait in a second file?
step 3: enhance the trip stuff to have a concept of hardcoded legs, and make it choose how to use a bus
	- seed a trip using a bus
	- test a basic bus scenario
	- make BusStop a lightweight, copyable address

loading GTFS is actually a bit unclear -- both the map layer and the sim layer
have to parse the same GTFS? well, it's like RoadEdits -- we can pass it into
the map constructor and physically just load it once.

routing: https://stackoverflow.com/questions/483488/strategy-to-find-your-best-route-via-public-transportation-only
- how do we indicate that the trip uses a bus stop? how will this actually get used?
	- in helpers, start trip from/to bldg maybe using transit. pathfind first using transit, then figure out the sequence of bus stops from the route, and turn that into the trip.
	- so ideally it's easy to know (stop1, route, stop2) almost as a step of the path. can translate that into trip legs pretty easily.
	- feels like a different interface, especially because the point is to just generate the stuff for the trip manager. throwing away the rest of the pathfinding stuff! hmm. same algorithm doesn't fit well at all.
