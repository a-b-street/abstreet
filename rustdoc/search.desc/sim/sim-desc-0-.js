searchState.loadedDescShard("sim", 0, "The sim crate runs a traffic simulation on top of the …\nQueued behind someone, or someone’s doing a conflicting …\nAs a simulation runs, different pieces emit Events. The …\nA pedestrian crossed an intersection with an Arterial …\nThe bikeable position\nPrint the alert to STDOUT and don’t proceed until the UI …\nThe number of active vehicles and commuters, broken into …\nA cyclist crossed an intersection with &gt;4 connecting roads.\nDon’t actually know where this goes yet!\nWhy is an agent delayed? If there are multiple reasons, …\nAt all speeds (including at rest), cars must be at least …\nShows an agent’s current inner intention or thoughts.\nWaiting on a traffic signal to change, or pausing at a …\nA vehicle waited &gt;30s, or a pedestrian waited &gt;15s.\nBuilding and idx (pretty meaningless)\nLane and idx\nAnother vehicle wanted to over-take this cyclist somewhere …\nNote that for offstreet parking, the path will be the same …\nToo many people are crossing the same sidewalk or …\nJust print the alert to STDOUT\nWhat stop did they board at?\nWhen spawning at borders, start the front of the vehicle …\nbool is contraflow\nPoint of interest, that is\nDon’t do anything\nThe Sim ties together all the pieces of the simulation. …\nSimFlags specifies a simulation to setup. After parsing …\nOptions controlling the traffic simulation.\nA sliding window, used to count something over time\nWhen a warning is encountered during simulation, specifies …\nAllow a vehicle to start a turn, even if their target lane …\nPretty hacky case\nMost fields in Analytics are cumulative over time, but …\nScheduled departure; the start may be delayed if the …\nAllow all agents to immediately proceed into an …\nNormally if a cycle of vehicles depending on each other to …\nDisable experimental handling for “uber-turns”, …\nNormally as a vehicle follows a route, it …\nEnable an experimental SEIR pandemic model. This requires …\nFinish time, ID, mode, trip duration if successful (or …\nNeed to explain this trick – basically keeps consistency …\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nThe numeric ID must be globally unique, without …\nIgnore parking data in the map and instead treat every …\nOnly for traffic signals. The u8 is the movement index …\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nThe same as <code>load_path</code>, but with a default value filled …\nA path to some file:\nEverything needed to setup a simulation.\nDid a ScenarioModifier apply to this?\nFront of the car\nAn experimental SEIR model by …\nTrue only for cars currently looking for parking. I don’…\nPer parking lane or lot, when does a spot become filled …\nPossibly the rest\nFor each passenger boarding, how long did they wait at the …\nNone means a bus.\nNone means a bus or parked car. Note parked cars do NOT …\nRecord different problems that each trip encounters.\nDesigned in …\nFor benchmarking, we may want to disable collecting data.\nIntermediate structures so that sim and game crates don’…\nAn arbitrary number to seed the random number generator. …\nFor vehicles only, not pedestrians. Follows a Path from …\nUsed to distinguish savestates for running the same …\nA JSON list of modifiers to transform the scenario. These …\nDon’t collect any analytics. Only useful for …\nNone for buses\nIgnore all stop signs and traffic signals, instead using a …\nVehicleType is bundled for convenience; many places need …\nBoth cars and bikes\nAs a simulation runs, different pieces emit Events. The …\nA pedestrian crossed an intersection with an Arterial …\nA cyclist crossed an intersection with &gt;4 connecting roads.\nA vehicle waited &gt;30s, or a pedestrian waited &gt;15s.\nAnother vehicle wanted to over-take this cyclist somewhere …\nToo many people are crossing the same sidewalk or …\nA sliding window, used to count something over time\nSee https://github.com/a-b-street/abstreet/issues/85\nReturns the count at time\nReturns pairs of trip times for finished trips in both …\nEnsure the points cover up to <code>end_time</code>. The last event may …\nGrab the count at this time, but don’t add a new time\n(Road or intersection, type, hour block) -&gt; count for that …\nMost fields in Analytics are cumulative over time, but …\nReturns the contents of a CSV file\nIgnores the current time. Returns None for cancelled trips.\nFinish time, ID, mode, trip duration if successful (or …\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nIf calling on prebaked Analytics, be careful to pass in an …\nOnly for traffic signals. The u8 is the movement index …\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nReturns the free spots over time\nPer parking lane or lot, when does a spot become filled …\nFor each passenger boarding, how long did they wait at the …\nReturns the rough location where the problem occurred – …\nRecord different problems that each trip encounters.\nVery expensive to store, so it’s optional. But useful to …\nFor benchmarking, we may want to disable collecting data.\nIf the agent is a transit vehicle, then include a count of …\nAs a simulation runs, different systems emit Events. This …\nTripID, TurnID (Where the delay was encountered), Time …\nHow long waiting at the stop?\nJust use for parking replanning. Not happy about copying …\nNone if cancelled\nWhat stop did they board at?\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nIntermediate structures used to instantiate a Scenario. …\nSimFlags specifies a simulation to setup. After parsing …\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nThe same as <code>load_path</code>, but with a default value filled …\nA path to some file:\nLoads a map and simulation. Not appropriate for use in the …\nAn arbitrary number to seed the random number generator. …\nA JSON list of modifiers to transform the scenario. These …\nSomething went wrong spawning the trip.\nWe need to remember a few things from scenario …\nCan be used to spawn from a border or anywhere for …\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nTurn an origin/destination pair and mode into a specific …\nThis must be a currently parked vehicle owned by the …\nThis must be a currently off-map vehicle owned by the …\nRepresents a single vehicle. Note “car” is a misnomer; …\nSee …\nWhere’s the front of the car while this is happening?\nAssumes the current head of the path is the thing to cross.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nIn reverse order – most recently left is first. The sum …\nNone for buses\nSince lane over-taking isn’t implemented yet, a vehicle …\nSimulates vehicles!\nAbruptly remove a vehicle from the simulation. They may be …\nFinds vehicles that’re laggy heads on affected parts of …\nReturns the argument unchanged.\nNote the ordering of results is non-deterministic!\nThis is about as expensive as get_draw_cars_on.\nNote the ordering of results is non-deterministic!\nCalls <code>U::from(self)</code>.\nIf the car wants to over-take somebody, what adjacent lane …\nNone if it worked, otherwise returns the CreateCar …\nState transitions for this car:\nAfter a leader (maybe an active vehicle, maybe a static …\nIf start_car_on_lane fails and a retry is scheduled, this …\nDoes the given car want to over-take the vehicle in front …\nManages conflicts at intersections. When an agent has …\nFor deleting cars\nSee if any agent is currently performing a turn that …\nReturns intersections with travelers waiting for at least …\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nFor cars: The head car calls this when they’re at the …\nThis is only triggered for traffic signals.\nVanished at border, stopped biking, etc – a vehicle …\nThis assigns infinite private parking to all buildings and …\nManages the state of parked cars. There are two …\nThere’s no DrawCarInput for cars parked offstreet, so we …\nThere’s no DrawCarInput for cars parked offstreet, so we …\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nThe vehicle’s front is currently at the given …\nThe vehicle’s front is currently at the given …\n(Filled, available)\n(Filled, available)\nReturns any cars that got very abruptly evicted from …\nReturns any cars that got very abruptly evicted from …\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCounterintuitive: any spots located in blackholes are just …\nUnrealistically assumes the driver has knowledge of …\nUnrealistically assumes the driver has knowledge of …\nNeeded when abruptly deleting a car, in case they’re …\nNeeded when abruptly deleting a car, in case they’re …\nThis follows whatever’s in front of it\nA Queue of vehicles on a single lane or turn. This is where\nThe exact position of something in a <code>Queue</code> at some time\nA member of a <code>Queue</code>.\nSomething occupying a fixed interval of distance on the …\nA regular vehicle trying to move forwards\nRecord that a car is blocking a static portion of the …\nNot including FOLLOWING_DISTANCE\nTrue if a static blockage can be inserted into the queue …\nRecord that a car is no longer blocking a dynamic portion …\nRecord that a car is no longer blocking a static portion …\nOnce a car has fully exited a queue, free up the space it …\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nHow long the lane or turn physically is.\nGet all cars in the queue, not including the laggy head or …\nReturn the exact position of each member of the queue. The …\nIf the specified car can appear in the queue, return the …\nGet the front of the last car in the queue.\nFind the vehicle in front of the specified input. None if …\nReturns the front of the last car in the queue, only if …\nRecord that a car has entered a queue at a position. This …\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nIf a car thinks it’s reached the end of the queue, …\nTrue if the reserved length exceeds the physical length. …\nThis car’s back is still partly in this queue.\nChange the first car in the queue to the laggy head, …\nRecord that a car has entered a queue at the end. It’s …\nRemove a car from a position. Need to separately do …\nRecord that a car is starting to change lanes away from …\nWhen a car’s turn is accepted, reserve the vehicle …\nCan a car start a turn for this queue?\nReturn a penalty for entering this queue, as opposed to …\nIf true, there’s room and the car must actually start …\nThis vehicle is exiting a driveway and cutting across a …\nThis vehicle is in the middle of changing lanes\nThe Distance is either 0 or the current traversable’s …\nSimulates pedestrians. Unlike vehicles, pedestrians can …\nReturns a number in (0, 1] to multiply speed by to account …\nAbruptly remove a pedestrian from the simulation. They may …\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nSimulate a curated list of scenarios to completion, and …\nRecords trips beginning and ending at a specified set of …\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nShows an agent’s current inner intention or thoughts.\nbool is contraflow\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nFront of the car\nTrue only for cars currently looking for parking. I don’…\nPossibly the rest\nNone means a bus.\nNone means a bus or parked car. Note parked cars do NOT …\nSpot and cached distance along the last driving lane\nReturns the step just finished\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalled when the car is Queued at the last step, or when …\nFront is always the current step\nNo parking available at all!\nA smaller version of Command that satisfies many more …\nThe priority queue driving the discrete event simulation. …\nA more compressed form of CommandType, just used for …\nIf true, retry when there’s no room to spawn somewhere\nThe Time is redundant, just used to dedupe commands\nDistinguish this from UpdateCar to avoid confusing things\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nThis API is safer than handing out a batch of items at a …\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nThis next command might’ve actually been rescheduled to …\nPrint the alert to STDOUT and don’t proceed until the UI …\nJust print the alert to STDOUT\nDon’t do anything\nThe Sim ties together all the pieces of the simulation. …\nOptions controlling the traffic simulation.\nOnly call for active agents, will panic otherwise\nWhen a warning is encountered during simulation, specifies …\nFor every parked car, (position of parking spot, position …\nAllow a vehicle to start a turn, even if their target lane …\nReturn a short string to debug a car in the UI.\nFor intersections with an agent waiting beyond some …\nAllow all agents to immediately proceed into an …\nNormally if a cycle of vehicles depending on each other to …\nDisable experimental handling for “uber-turns”, …\nNormally as a vehicle follows a route, it …\nReturns a boxed object from a boxed trait object if the …\nReturns a mutable reference to the object within the trait …\nReturns an <code>Rc</code>-ed object from an <code>Rc</code>-ed trait object if the …\nReturns a reference to the object within the trait object …\nEnable an experimental SEIR pandemic model. This requires …\nReturns (trips affected, number of parked cars displaced)\nIf trip is finished, returns (total time, total waiting …\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\n(Filled, available)\nFor every agent that’s currently not moving, figure out …\nReturns people / m^2. Roads have up to two sidewalks and …\nReturns the best-case time for a trip in a world with no …\nThis does not include transit riders. Some callers need …\nRespond to arbitrary map edits without resetting the …\nIf present, live map edits are being processed, and the …\nIgnore parking data in the map and instead treat every …\nIf retry_if_no_room is false, any vehicles that fail to …\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nReturns true if the trait object wraps an object of type …\n(total number of people, just in buildings, just off map)\n(number of finished trips, number of unfinished trips)\nAll sorts of read-only queries about a simulation\nUsed to distinguish savestates for running the same …\nOnly one at a time supported.\nDon’t collect any analytics. Only useful for …\n(bus, stop index it’s coming from, percent to next stop, …\n(number of vehicles in the lane, penalty if a bike or …\nIgnore all stop signs and traffic signals, instead using a …\nQueued behind someone, or someone’s doing a conflicting …\nWhy is an agent delayed? If there are multiple reasons, …\nWaiting on a traffic signal to change, or pausing at a …\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nManages public transit vehicles (buses and trains) that …\n(buses, trains)\nIf true, the bus is idling. If false, the bus actually …\nalso stop idx that the bus is coming from\nReturns the path for the first leg.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nWhere does each passenger want to deboard?\nReturns the bus if the pedestrian boarded immediately.\nwaiting at =&gt; (ped, route, bound for, started waiting)\nThe number of active vehicles and commuters, broken into …\nA person may own many vehicles, so specify which they use\nMaybe get off at a stop, maybe ride off-map\nThese don’t specify where the leg starts, since it might …\nManages people, each of which executes some trips through …\nThis is idempotent to handle the case of cars retrying …\nThis will be None for parked cars and buses. Should always …\nCancel a trip after it’s started. The person will be …\nCancel a trip before it’s started. The person will stay …\nScheduled departure; the start may be delayed if the …\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nRecreate the Scenario from an instantiated simulation. The …\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nDid a ScenarioModifier apply to this?\nIf no route is returned, the pedestrian boarded a bus …\nBoth cars and bikes")