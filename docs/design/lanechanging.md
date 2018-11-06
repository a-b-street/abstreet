# Lane-changing

Ah, this bundle of joy.

Steps:
- query to know when is it safe (similar to spawn checking)
- the state to execute it (and assert it completes in time)
- the modifications to lookahead
- change pathfinding to understand new movements
	- how many LCs can be done on one road?
	- how to render it?
- decide if LCing can ever fail (or if cars will just stop and wait to merge)
- change what turns are initially created
