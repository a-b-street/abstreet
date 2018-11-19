# Physics-related design notes

## Floating point and units

Currently using si::Second<f64> for time, which means comparing sim state by
deriving Eq is a headache. Since the timestep size is fixed anyway, this should
just become ticks. Was tempted to use usize, but arch dependence is weird, and
with a 0.1s timestep, 2^32 - 1 ticks is about 13.5 years, which is quite a long
timescale for a traffic simulation. :) So, let's switch to u32.

Now I'm hitting all the fun FP imprecision issues. Could probably hack in
epsilon and negative checks everywhere in kinematics, but why? Should research
it more, but I think the better approach is to use fixed-point arithmetic for
everything (aka u8 or whatever).

Problems with floats:
- they dont natively order
- they arent PartialEq
- serialization sometimes breaks
- epsilon comparison issues

Options:
	- just to solve epsilon comparison issues
		- https://crates.io/crates/float-cmp
		- https://crates.io/crates/approx
	- just to get Ord and Eq
		- https://crates.io/crates/decorum
		- https://crates.io/crates/ordered-float
	- use rational / fraction types
		- https://crates.io/crates/fraction
		- https://crates.io/crates/rug
		- this seems a bit overpowered
	- use my own custom wrapper type around u8 or whatever size makes sense for each thing
		- representing 0.1s or 0.1m or whatever
	- use a fixed point arithmetic crate
		- https://crates.io/crates/fpa
		- https://crates.io/crates/fixed
		- https://crates.io/crates/fix
		- would want to wrap the types anyway; only some operations make sense


- can we compare results with the prev float approach? make them store the
  other thing, compare results? or compare the results of a sim?
- is it possible to do this change gradually? unlikely...
	- stick all the types in geom, for now


- moment in time (tick)
	- resolution: 0.1s with u32, so about 13.5 years
- duration
	- resolution: 0.1s with u32, so about 13.5 years
	- time - time = duration
- distance (always an interval -- dist_along is relative to the start)
	- non-negative by construction!
	- say resolution is 0.3m (about 1ft), use u32, huge huge distances
	- geometry is polylines, sequences of (f64, f64) representing meters
	  from some origin. we could keep drawing the same, but do stuff like
	  dist_along as this new type? or change Pt2D to have more reasonable resolution?
	- still represent angles as f64 radians? for drawing, turn calculation, etc
- speed
	- meters per second, constructed from distance / duration
	- should resolution be 0.3m / 0.1s? 1 unit of distance per one timestep? or, maybe less than that?
	- can be negative
- acceleration
	- can be negative
	- what should the resolution be?

## Types again

Alright, very annoyed about hacking around PartialEq support for dimensioned
stuff everywhere. And dimensioned is providing no benefit -- I'd prefer my own
types. Can also add in validation that there are no negative distances. Also
the constants aren't particularly nice.

One thing -- f64 is PartialEq, not Eq. Why do I need Eq? sim determinism works
fine with with PartialEq. Undid tons of stuff, woot!

Ah, that was easy. Still reasons to make my own native types:
- less dependencies
- explicit operations supported, makes code more clear
- can verify distance is non-negative
- can maybe make the epsilon tolerance consistent

## Coordinate system

Switching to something that's more easily bijective, but
https://crates.io/crates/flat_projection could be a good candidate too.
