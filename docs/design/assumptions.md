# Assumptions

aka things to verify up-front before bugs creep into sim layers

## Map model

- no short lanes
	- exactly what's the limit or problem?
	- how should dog-leg intersections be modeled?
	- maybe an easy baseline: a parking or driving lane that cant fit one max vehicle length
- connectivity
	- from any sidewalk to any other
	- from any driving lane to any other
- parking spots and bus stops line up with driving lane reasonably; dont exceed length
- associated lanes
	- parking lane or bus stop without driving lane
