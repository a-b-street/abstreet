use crate::{line_intersection, Angle, Bounds, Line, Polygon, Pt2D, Triangle, EPSILON_DIST};
use dimensioned::si;
use ordered_float::NotNaN;
use serde_derive::{Deserialize, Serialize};
use std::f64;
use std::fmt;

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
        if start >= end || start < 0.0 * si::M || end < 0.0 * si::M {
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

    pub fn middle(&self) -> Pt2D {
        self.safe_dist_along(self.length() / 2.0).unwrap().0
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

    pub fn dashed_polygons(
        &self,
        width: f64,
        dash_len: si::Meter<f64>,
        dash_separation: si::Meter<f64>,
    ) -> Vec<Polygon> {
        let mut polygons: Vec<Polygon> = Vec::new();

        let total_length = self.length();

        let mut start = 0.0 * si::M;
        loop {
            if start + dash_len >= total_length {
                break;
            }

            polygons.push(
                self.slice(start, start + dash_len)
                    .0
                    .make_polygons_blindly(width),
            );
            start += dash_len + dash_separation;
        }

        polygons
    }

    pub fn intersection(&self, other: &PolyLine) -> Option<Pt2D> {
        assert_ne!(self, other);

        // There could be several collisions. Pick the "first" from self's perspective.
        for l1 in self.lines() {
            let mut hits: Vec<Pt2D> = Vec::new();
            for l2 in other.lines() {
                if let Some(pt) = l1.intersection(&l2) {
                    hits.push(pt);
                }
            }

            hits.sort_by_key(|pt| {
                let mut copy = self.clone();
                copy.trim_to_pt(*pt);
                NotNaN::new(copy.length().value_unsafe).unwrap()
            });
            if !hits.is_empty() {
                return Some(hits[0]);
            }
        }
        None
    }

    pub fn intersection_infinite_line(&self, other: Line) -> Option<Pt2D> {
        // TODO There must be better ways to do this. :)
        for l in self.lines() {
            if let Some(hit) = line_intersection(&l, &other) {
                if l.contains_pt(hit) {
                    return Some(hit);
                }
            }
        }
        None
    }

    // Starts trimming from the head. Panics if the pt is not on the polyline.
    pub fn trim_to_pt(&mut self, pt: Pt2D) {
        if let Some(idx) = self.lines().iter().position(|l| l.contains_pt(pt)) {
            self.pts.truncate(idx + 1);
            self.pts.push(pt);
        } else {
            panic!("Can't trim_to_pt: {} doesn't contain {}", self, pt);
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

    pub fn get_bounds(&self) -> Bounds {
        let mut b = Bounds::new();
        for pt in &self.pts {
            b.update(*pt);
        }
        b
    }
}

impl fmt::Display for PolyLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "PolyLine::new(vec![")?;
        for (idx, pt) in self.pts.iter().enumerate() {
            write!(f, "  Pt2D::new({}, {}),", pt.x(), pt.y())?;
            if idx > 0 {
                write!(
                    f,
                    "    // {}, {}",
                    pt.x() - self.pts[idx - 1].x(),
                    pt.y() - self.pts[idx - 1].y()
                )?;
            }
            writeln!(f)?;
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
