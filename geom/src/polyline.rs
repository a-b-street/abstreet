use dimensioned::si;
use std::f64;
use std::fmt;
use {line_intersection, Angle, Line, Polygon, Pt2D, Triangle, EPSILON_DIST};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PolyLine {
    pts: Vec<Pt2D>,
}

impl PolyLine {
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

    pub fn extend(&mut self, other: PolyLine) {
        assert_eq!(*self.pts.last().unwrap(), other.pts[0]);
        self.pts.extend(other.pts.iter().skip(1));
    }

    pub fn points(&self) -> &Vec<Pt2D> {
        &self.pts
    }

    // Makes a copy :\
    pub fn lines(&self) -> Vec<Line> {
        self.pts
            .windows(2)
            .map(|pair| Line::new(pair[0], pair[1]))
            .collect()
    }

    pub fn length(&self) -> si::Meter<f64> {
        self.lines()
            .iter()
            .fold(0.0 * si::M, |so_far, l| so_far + l.length())
    }

    // Returns the excess distance left over from the end.
    pub fn slice(&self, start: si::Meter<f64>, end: si::Meter<f64>) -> (PolyLine, si::Meter<f64>) {
        if start >= end {
            panic!("Can't get a polyline slice [{}, {}]", start, end);
        }

        let mut result: Vec<Pt2D> = Vec::new();
        let mut dist_so_far = 0.0 * si::M;

        for line in self.lines().iter() {
            let length = line.length();

            // Does this line contain the first point of the slice?
            if result.is_empty() && dist_so_far + length >= start {
                result.push(line.dist_along(start - dist_so_far));
            }

            // Does this line contain the last point of the slice?
            if dist_so_far + length >= end {
                result.push(line.dist_along(end - dist_so_far));
                return (PolyLine::new(result), 0.0 * si::M);
            }

            // If we're in the middle, just collect the endpoint.
            if !result.is_empty() {
                result.push(line.pt2());
            }

            dist_so_far += length;
        }

        if result.is_empty() {
            panic!(
                "Slice [{}, {}] has a start too big for polyline of length {}",
                start,
                end,
                self.length()
            );
        }

        (PolyLine::new(result), end - dist_so_far)
    }

    // TODO return result with an error message
    pub fn safe_dist_along(&self, dist_along: si::Meter<f64>) -> Option<(Pt2D, Angle)> {
        if dist_along < 0.0 * si::M {
            return None;
        }

        let mut dist_left = dist_along;
        for (idx, l) in self.lines().iter().enumerate() {
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
        // TODO Grrr, the new algorithm actually breaks pretty badly on medium. Disable it for now.
        if true {
            return self.shift_blindly_with_sharp_angles(width);
        }

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

            // If the two line SEGMENTS intersected, then just use that one point.
            if l1.intersects(&l2) {
                result.push(pt2_shift);
            } else {
                // Otherwise, the line intersection will occur farther than width away from the
                // original pt2_raw. At various angles, this explodes out way too much. So insert a
                // few points to make the corner nicer.
                result.push(l1.pt2());
                result.push(Line::new(pt2_raw, pt2_shift).dist_along(width * si::M));
                result.push(l2.pt1());
            }

            if pt3_idx == self.pts.len() - 1 {
                result.push(l2.pt2());
                break;
            }

            pt1_raw = pt2_raw;
            pt2_raw = pt3_raw;
            pt3_idx += 1;
        }

        // Might have extra points to handle sharp bends
        assert!(result.len() >= self.pts.len());
        PolyLine::new(result)
    }

    // Shifting might fail if the requested width doesn't fit in tight angles between points in the
    // polyline.
    pub fn shift(&self, width: f64) -> Option<PolyLine> {
        let result = self.shift_blindly(width);
        // TODO check if any non-adjacent line segments intersect
        Some(result)
    }

    // Doesn't massage sharp twists into more points. For polygon rendering.
    fn shift_blindly_with_sharp_angles(&self, width: f64) -> PolyLine {
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

    // Doesn't massage sharp twists into more points. For polygon rendering. Shifting might fail if
    // the requested width doesn't fit in tight angles between points in the polyline.
    fn shift_with_sharp_angles(&self, width: f64) -> Option<PolyLine> {
        let result = self.shift_blindly(width);

        // Check that the angles roughly match up between the original and shifted line
        for (orig_l, shifted_l) in self.lines().iter().zip(result.lines().iter()) {
            let orig_angle = orig_l.angle().normalized_degrees();
            let shifted_angle = shifted_l.angle().normalized_degrees();
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

    // This could fail by needing too much width for sharp angles
    pub fn make_polygons(&self, width: f64) -> Option<Polygon> {
        let side1 = self.shift_with_sharp_angles(width / 2.0)?;
        let side2 = self
            .reversed()
            .shift_with_sharp_angles(width / 2.0)?
            .reversed();
        Some(self.polygons_from_sides(&side1, &side2))
    }

    pub fn make_polygons_blindly(&self, width: f64) -> Polygon {
        let side1 = self.shift_blindly_with_sharp_angles(width / 2.0);
        let side2 = self
            .reversed()
            .shift_blindly_with_sharp_angles(width / 2.0)
            .reversed();
        self.polygons_from_sides(&side1, &side2)
    }

    fn polygons_from_sides(&self, side1: &PolyLine, side2: &PolyLine) -> Polygon {
        let mut poly = Polygon {
            triangles: Vec::new(),
        };
        for high_idx in 1..self.pts.len() {
            // Duplicate first point, since that's what graphics layer expects
            poly.triangles.push(Triangle::new(
                side1.pts[high_idx],
                side1.pts[high_idx - 1],
                side2.pts[high_idx - 1],
            ));
            poly.triangles.push(Triangle::new(
                side2.pts[high_idx],
                side2.pts[high_idx - 1],
                side1.pts[high_idx],
            ));
        }
        poly
    }

    pub fn intersection(&self, other: &PolyLine) -> Option<Pt2D> {
        assert_ne!(self, other);

        for l1 in self.lines() {
            for l2 in other.lines() {
                if let Some(pt) = l1.intersection(&l2) {
                    return Some(pt);
                }
            }
        }
        None
    }

    // Starts trimming from the head. If the pt is not on the polyline, returns false -- but this
    // is a bug somewhere else.
    pub fn trim_to_pt(&mut self, pt: Pt2D) -> bool {
        if let Some(idx) = self.lines().iter().position(|l| l.contains_pt(pt)) {
            self.pts.truncate(idx + 1);
            self.pts.push(pt);
            true
        } else {
            println!("{} doesn't contain {}", self, pt);
            false
        }
    }

    pub fn dist_along_of_point(&self, pt: Pt2D) -> Option<si::Meter<f64>> {
        let mut dist_along = 0.0 * si::M;
        for l in self.lines() {
            if let Some(dist) = l.dist_along_of_point(pt) {
                return Some(dist_along + dist);
            } else {
                dist_along += l.length();
            }
        }
        None
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

#[test]
fn trim_with_epsilon() {
    /*
    // EPSILON_DIST needs to be tuned correctly, or this point seems like it's not on the line.
    let mut pl = PolyLine::new(vec![
      Pt2D::new(1130.2653468611902, 2124.099702776818),
      Pt2D::new(1175.9652436108408, 2124.1094748373457),
      Pt2D::new(1225.8319649025132, 2124.120594334445),
    ]);
    let pt = Pt2D::new(1225.8319721124885, 2124.1205943360505);*/

    let mut pl = PolyLine::new(vec![
        Pt2D::new(1725.295220788561, 1414.2752785686052),
        Pt2D::new(1724.6291929910137, 1414.8246144364846),
        Pt2D::new(1723.888820814687, 1415.6240169312443),
        Pt2D::new(1723.276510998312, 1416.4750455089877),
        Pt2D::new(1722.7586731922217, 1417.4015448461048),
        Pt2D::new(1722.353627188061, 1418.4238284182732),
        Pt2D::new(1722.086748762076, 1419.4737997607863),
        Pt2D::new(1721.9540106814163, 1420.5379609077854),
        Pt2D::new(1721.954010681534, 1421.1267599802409),
    ]);
    let pt = Pt2D::new(1721.9540106813197, 1420.2372293808348);

    pl.trim_to_pt(pt);
}

// TODO test that shifting lines and polylines is a reversible operation
