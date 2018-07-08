# TODO for Phase 3 (Simulation)

## cars

- model cars parking
	- maybe render numbers on the cars to distinguish them
	- document the FSM (on lane driving, waiting, turning, parking, etc)
	- populate a bunch of parked cars initially

- try to simplify straw_model step (less phases?)

- better visualization
	- draw moving / blocked colors (gradually more red as they wait longer)
	- draw stop buffer in front/behind of cars
	- draw cars in intersections, even when slightly zoomed out
	- draw cars in slightly different colors, to distinguish them better

- start implementing a second AORTAish driving model

- reversible sim

## bikes

- model bikes in driving lanes (as slow cars)

## pedestrians

- model pedestrians
	- also move from building to sidewalk?
