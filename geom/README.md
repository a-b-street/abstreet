# geom

This crate contains primitive types used by A/B Street. It's unclear if other
apps will have any use for this crate. In some cases, `geom` just wraps much
more polished APIs, like `rust-geo`. In others, it has its own geometric
algorithms, but they likely have many bugs and make use-case-driven assumptions.
So, be warned if you use this.

## Contents

Many of the types are geometric: `Pt2D`, `Ring`, `Distance`, `Line`,
`InfiniteLine`, `FindClosest`, `Circle`, `Angle`, `LonLat`, `Bounds`,
`GPSBounds`, `PolyLine`, `Polygon`, `Triangle`.

Some involve time: `Time`, `Duration`, `Speed`.

And there's also a `Percent` wrapper and a `Histogram`.
