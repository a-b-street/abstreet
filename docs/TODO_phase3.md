# TODO for Phase 3 (Simulation)

## cars

- model cars parking
	- render parked cars (different color)
	- make vanished cars just park again
		- how to handle when parking is full or no parking at goal road?

- code cleanup
	- try to simplify straw_model step (less phases?)
	- figure out responsibility btwn agents and managers, then fix up visibility
	- rng should live in a scenario spec layer, not in the sim itself

- better visualization
	- draw moving / blocked colors (gradually more red as they wait longer)
	- draw stop buffer in front/behind of cars
	- draw cars in intersections, even when slightly zoomed out
	- draw cars in slightly different colors, to distinguish them better

- start implementing a second AORTAish driving model
	- then make cars park/unpark at the correct position

- reversible sim

## bikes

- model bikes in driving lanes (as slow cars)

## pedestrians

- model pedestrians
	- also move from building to sidewalk?
