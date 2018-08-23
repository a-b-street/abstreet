use graphics::math::Vec2d;
use ordered_float::NotNaN;
use std::f64;
use std::fmt;
use {Angle, Bounds, LonLat};

// This represents world-space in meters.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pt2D {
    x: f64,
    y: f64,
}

impl Pt2D {
    pub fn new(x: f64, y: f64) -> Pt2D {
        // TODO enforce after fixing:
        // - shift_polyline goes OOB sometimes
        // - convert_map uses this for GPS I think?
        if x < 0.0 || y < 0.0 {
            //println!("Bad pt: {}, {}", x, y);
        }
        //assert!(x >= 0.0);
        //assert!(y >= 0.0);

        Pt2D { x, y }
    }

    pub fn from_gps(gps: &LonLat, b: &Bounds) -> Pt2D {
        // TODO hack to construct test maps more easily
        if b.represents_world_space {
            return Pt2D::new(gps.longitude, gps.latitude);
        }

        // If not, havoc ensues
        assert!(b.contains(gps.longitude, gps.latitude));

        // Invert y, so that the northernmost latitude is 0. Screen drawing order, not Cartesian grid.
        let base = LonLat::new(b.min_x, b.max_y);

        // Apparently the aabb_quadtree can't handle 0, so add a bit.
        // TODO epsilon or epsilon - 1.0?
        let dx = base.gps_dist_meters(LonLat::new(gps.longitude, base.latitude)) + f64::EPSILON;
        let dy = base.gps_dist_meters(LonLat::new(base.longitude, gps.latitude)) + f64::EPSILON;
        // By default, 1 meter is one pixel. Normal zooming can change that. If we did scaling here,
        // then we'd have to update all of the other constants too.
        Pt2D::new(dx, dy)
    }

    pub fn x(&self) -> f64 {
        self.x
    }

    pub fn y(&self) -> f64 {
        self.y
    }

    // TODO probably remove this
    pub fn to_vec(&self) -> Vec2d {
        [self.x(), self.y()]
    }

    // TODO better name
    // TODO Meters for dist?
    pub fn project_away(&self, dist: f64, theta: Angle) -> Pt2D {
        // If negative, caller should use theta.opposite()
        assert!(dist >= 0.0);

        let (sin, cos) = theta.normalized_radians().sin_cos();
        Pt2D::new(self.x() + dist * cos, self.y() + dist * sin)
    }

    pub fn angle_to(&self, to: Pt2D) -> Angle {
        // DON'T invert y here
        Angle::new((to.y() - self.y()).atan2(to.x() - self.x()))
    }

    pub fn offset(&self, dx: f64, dy: f64) -> Pt2D {
        Pt2D::new(self.x() + dx, self.y() + dy)
    }
}

impl fmt::Display for Pt2D {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Pt2D({0}, {1})", self.x(), self.y())
    }
}

// This isn't opinionated about what the (x, y) represents -- could be lat/lon or world space.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct HashablePt2D {
    x_nan: NotNaN<f64>,
    y_nan: NotNaN<f64>,
}

impl HashablePt2D {
    pub fn new(x: f64, y: f64) -> HashablePt2D {
        HashablePt2D {
            x_nan: NotNaN::new(x).unwrap(),
            y_nan: NotNaN::new(y).unwrap(),
        }
    }

    pub fn x(&self) -> f64 {
        self.x_nan.into_inner()
    }

    pub fn y(&self) -> f64 {
        self.y_nan.into_inner()
    }
}

impl From<Pt2D> for HashablePt2D {
    fn from(pt: Pt2D) -> Self {
        HashablePt2D::new(pt.x(), pt.y())
    }
}
