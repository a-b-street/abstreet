use dimensioned::si;
use graphics::math::Vec2d;
use std::f64;
use std::fmt;
use {line_intersection, Angle, Line, Pt2D, EPSILON_DIST};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PolyLine {
    pts: Vec<Pt2D>,
}

impl PolyLine {
    // TODO helper for lines() would be nice, so we dont have to spam windows(2) and deal with
    // pairs

    pub fn new(pts: Vec<Pt2D>) -> PolyLine {
        assert!(pts.len() >= 2);
        PolyLine { pts }
    }

    // TODO copy or mut?
    // TODO this is likely not needed if we just have a way to shift in the other direction
    pub fn reversed(&self) -> PolyLine {
        let mut pts = self.pts.clone();
        pts.reverse();
        PolyLine::new(pts)
    }

    // TODO weird to have these specific things?
    pub fn replace_first_line(&mut self, pt1: Pt2D, pt2: Pt2D) {
        self.pts[0] = pt1;
        self.pts[1] = pt2;
    }

    pub fn replace_last_line(&mut self, pt1: Pt2D, pt2: Pt2D) {
        let len = self.pts.len();
        self.pts[len - 2] = pt1;
        self.pts[len - 1] = pt2;
    }

    pub fn points(&self) -> &Vec<Pt2D> {
        &self.pts
    }

    pub fn length(&self) -> si::Meter<f64> {
        self.pts.windows(2).fold(0.0 * si::M, |so_far, pair| {
            so_far + Line::new(pair[0], pair[1]).length()
        })
    }

    // TODO return result with an error message
    pub fn safe_dist_along(&self, dist_along: si::Meter<f64>) -> Option<(Pt2D, Angle)> {
        if dist_along < 0.0 * si::M {
            return None;
        }

        let mut dist_left = dist_along;
        for (idx, pair) in self.pts.windows(2).enumerate() {
            let l = Line::new(pair[0], pair[1]);
            let length = l.length();
            let epsilon = if idx == self.pts.len() - 2 {
                EPSILON_DIST
            } else {
                0.0 * si::M
            };
            if dist_left <= length + epsilon {
                return Some((l.dist_along(dist_left), l.angle()));
            }
            dist_left -= length;
        }
        None
    }

    // TODO rm this one
    pub fn dist_along(&self, dist_along: si::Meter<f64>) -> (Pt2D, Angle) {
        if let Some(pair) = self.safe_dist_along(dist_along) {
            return pair;
        }
        if dist_along < 0.0 * si::M {
            panic!("dist_along {} is negative", dist_along);
        }
        panic!("dist_along {} is longer than {}", dist_along, self.length());
    }

    pub fn first_pt(&self) -> Pt2D {
        self.pts[0]
    }
    pub fn last_pt(&self) -> Pt2D {
        *self.pts.last().unwrap()
    }
    pub fn first_line(&self) -> Line {
        Line::new(self.pts[0], self.pts[1])
    }
    pub fn last_line(&self) -> Line {
        Line::new(self.pts[self.pts.len() - 2], self.pts[self.pts.len() - 1])
    }

    // Doesn't check if the result is valid
    pub fn shift_blindly(&self, width: f64) -> PolyLine {
        if self.pts.len() == 2 {
            let l = Line::new(self.pts[0], self.pts[1]).shift(width);
            return PolyLine::new(vec![l.pt1(), l.pt2()]);
        }

        let mut result: Vec<Pt2D> = Vec::new();

        let mut pt3_idx = 2;
        let mut pt1_raw = self.pts[0];
        let mut pt2_raw = self.pts[1];

        loop {
            let pt3_raw = self.pts[pt3_idx];

            let l1 = Line::new(pt1_raw, pt2_raw).shift(width);
            let l2 = Line::new(pt2_raw, pt3_raw).shift(width);
            // When the lines are perfectly parallel, it means pt2_shift_1st == pt2_shift_2nd and the
            // original geometry is redundant.
            let pt2_shift = line_intersection(&l1, &l2).unwrap_or(l1.pt2());

            if pt3_idx == 2 {
                result.push(l1.pt1());
            }
            result.push(pt2_shift);
            if pt3_idx == self.pts.len() - 1 {
                result.push(l2.pt2());
                break;
            }

            pt1_raw = pt2_raw;
            pt2_raw = pt3_raw;
            pt3_idx += 1;
        }

        assert!(result.len() == self.pts.len());
        PolyLine::new(result)
    }

    // Shifting might fail if the requested width doesn't fit in tight angles between points in the
    // polyline.
    pub fn shift(&self, width: f64) -> Option<PolyLine> {
        let result = self.shift_blindly(width);

        // Check that the angles roughly match up between the original and shifted line
        for (orig_pair, shifted_pair) in self.pts.windows(2).zip(result.pts.windows(2)) {
            let orig_angle = orig_pair[0].angle_to(orig_pair[1]).normalized_degrees();
            let shifted_angle = shifted_pair[0]
                .angle_to(shifted_pair[1])
                .normalized_degrees();
            let delta = (shifted_angle - orig_angle).abs();
            if delta > 0.00001 {
                /*println!(
                    "Points changed angles from {} to {}",
                    orig_angle, shifted_angle
                );*/
                return None;
            }
        }
        Some(result)
    }

    // TODO why do we need a bunch of triangles? why doesn't the single polygon triangulate correctly?
    // TODO ideally, detect when the polygon overlaps itself due to sharp lines and too much width
    // return Vec2d since this is only used for drawing right now
    // Represent failure too
    pub fn make_polygons(&self, width: f64) -> Option<Vec<Vec<Vec2d>>> {
        let side1 = self.shift(width / 2.0)?;
        let side2 = self.reversed().shift(width / 2.0)?.reversed();
        Some(self.polygons_from_sides(&side1, &side2))
    }

    pub fn make_polygons_blindly(&self, width: f64) -> Vec<Vec<Vec2d>> {
        let side1 = self.shift_blindly(width / 2.0);
        let side2 = self.reversed().shift_blindly(width / 2.0).reversed();
        self.polygons_from_sides(&side1, &side2)
    }

    fn polygons_from_sides(&self, side1: &PolyLine, side2: &PolyLine) -> Vec<Vec<Vec2d>> {
        let mut result: Vec<Vec<Pt2D>> = Vec::new();
        for high_idx in 1..self.pts.len() {
            // Duplicate first point, since that's what graphics layer expects
            result.push(vec![
                side1.pts[high_idx],
                side1.pts[high_idx - 1],
                side2.pts[high_idx - 1],
                side1.pts[high_idx],
            ]);
            result.push(vec![
                side2.pts[high_idx],
                side2.pts[high_idx - 1],
                side1.pts[high_idx],
                side2.pts[high_idx],
            ]);
        }
        result
            .iter()
            .map(|pts| pts.iter().map(|pt| pt.to_vec()).collect())
            .collect()
    }

    pub fn intersection(&self, other: &PolyLine) -> Option<Pt2D> {
        for pair1 in self.pts.windows(2) {
            let l1 = Line::new(pair1[0], pair1[1]);
            for pair2 in other.pts.windows(2) {
                let l2 = Line::new(pair2[0], pair2[1]);
                if let Some(pt) = l1.intersection(&l2) {
                    return Some(pt);
                }
            }
        }
        None
    }

    // Starts trimming from the head. Assumes the pt is on the polyline somewhere.
    pub fn trim_to_pt(&mut self, pt: Pt2D) {
        if let Some(idx) = self.pts
            .windows(2)
            .position(|pair| Line::new(pair[0], pair[1]).contains_pt(pt))
        {
            self.pts.truncate(idx + 1);
            self.pts.push(pt);
        } else {
            panic!("{} doesn't contain {}", self, pt);
        }
    }
}

impl fmt::Display for PolyLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PolyLine::new(vec![\n")?;
        for pt in &self.pts {
            write!(f, "  Pt2D::new({}, {}),\n", pt.x(), pt.y())?;
        }
        write!(f, "])")
    }
}

// TODO unsure why this doesn't work. maybe see if mouse is inside polygon to check it out?
/*fn polygon_for_polyline(center_pts: &Vec<(f64, f64)>, width: f64) -> Vec<[f64; 2]> {
    let mut result = shift_polyline(width / 2.0, center_pts);
    let mut reversed_center_pts = center_pts.clone();
    reversed_center_pts.reverse();
    result.extend(shift_polyline(width / 2.0, &reversed_center_pts));
    // TODO unclear if piston needs last point to match the first or not
    let first_pt = result[0];
    result.push(first_pt);
    result.iter().map(|pair| [pair.0, pair.1]).collect()
}*/

#[test]
fn shift_polyline_equivalence() {
    use line_intersection;
    use rand;

    let scale = 1000.0;
    let pt1 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);
    let pt2 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);
    let pt3 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);
    let pt4 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);
    let pt5 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);

    let width = 50.0;
    let pt1_s = Line::new(pt1, pt2).shift(width).pt1();
    let pt2_s = line_intersection(
        &Line::new(pt1, pt2).shift(width),
        &Line::new(pt2, pt3).shift(width),
    ).unwrap();
    let pt3_s = line_intersection(
        &Line::new(pt2, pt3).shift(width),
        &Line::new(pt3, pt4).shift(width),
    ).unwrap();
    let pt4_s = line_intersection(
        &Line::new(pt3, pt4).shift(width),
        &Line::new(pt4, pt5).shift(width),
    ).unwrap();
    let pt5_s = Line::new(pt4, pt5).shift(width).pt2();

    assert_eq!(
        PolyLine::new(vec![pt1, pt2, pt3, pt4, pt5]).shift(width),
        Some(PolyLine::new(vec![pt1_s, pt2_s, pt3_s, pt4_s, pt5_s]))
    );
}

#[test]
fn shift_short_polyline_equivalence() {
    use rand;

    let scale = 1000.0;
    let pt1 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);
    let pt2 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);

    let width = 50.0;
    let l = Line::new(pt1, pt2).shift(width);

    assert_eq!(
        PolyLine::new(vec![pt1, pt2]).shift(width),
        Some(PolyLine::new(vec![l.pt1(), l.pt2()]))
    );
}

// TODO test that shifting lines and polylines is a reversible operation
