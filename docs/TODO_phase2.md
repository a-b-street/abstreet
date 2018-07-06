# TODO for Phase 2 (Editor)

- support massive maps
	- render to a bitmap and clip that in?
	- sometimes UI zooms in at once, then unzooms slowly. drop events?

- different UIs
	- 3D UI sharing the same structure as the 2D one
	- svg export some area, for manual mockups
	- web version
		- ggez, quicksilver, unrust could work

- easy UI bugs
	- big maps start centered over emptiness

- traffic signal editor
	- button to reset intersection to original cycles
	- turns can belong to multiple cycles; the colors become slightly meaningless
	- support left turn yield

- stop sign editor
	- cant have no stop signs for two roads whose center line crosses
		- infer default policy

- add random bike lanes, figure out how turns would work
- be able to convert between parking and bike lanes, recompute the turns
