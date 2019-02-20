use crate::{
    Angle, Bounds, Distance, HashablePt2D, InfiniteLine, Line, Polygon, Pt2D, EPSILON_DIST,
};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PolyLine {
    pts: Vec<Pt2D>,
    // TODO Note that caching length doesn't improve profiling results (by running
    // small_spawn_completes test in release mode). May not be worth doing this.
    length: Distance,
}

impl PolyLine {
    pub fn new(pts: Vec<Pt2D>) -> PolyLine {
        assert!(pts.len() >= 2);
        let length = pts.windows(2).fold(Distance::ZERO, |so_far, pair| {
            so_far + pair[0].dist_to(pair[1])
        });

        // This checks no lines are too small. Could take the other approach and automatically
        // squish down points here and make sure the final result is at least EPSILON_DIST.
        // But probably better for the callers to do this -- they have better understanding of what
        // needs to be squished down, why, and how.
        if pts.windows(2).any(|pair| pair[0].epsilon_eq(pair[1])) {
            panic!(
                "PL with total length {} and {} pts has ~dupe pts: {:?}",
                length,
                pts.len(),
                pts
            );
        }

        // Can't have duplicates! If the polyline ever crosses back on itself, all sorts of things
        // are broken.
        let seen_pts: HashSet<HashablePt2D> =
            pts.iter().map(|pt| HashablePt2D::from(*pt)).collect();
        if seen_pts.len() != pts.len() {
            panic!("PolyLine has repeat points: {:?}", pts);
        }

        PolyLine { pts, length }
    }

    pub fn make_polygons_for_boundary(pts: Vec<Pt2D>, thickness: Distance) -> Polygon {
        // Points WILL repeat -- fast-path some stuff.
        let pl = PolyLine {
            pts,
            length: Distance::ZERO,
        };
        pl.make_polygons(thickness)
    }

    pub fn reversed(&self) -> PolyLine {
        let mut pts = self.pts.clone();
        pts.reverse();
        PolyLine::new(pts)
    }

    // TODO Rename append, make a prepend that just flips the arguments
    pub fn extend(self, other: &PolyLine) -> PolyLine {
        assert_eq!(*self.pts.last().unwrap(), other.pts[0]);

        // There's an exciting edge case: the next point to add is on self's last line.
        let same_line = self
            .last_line()
            .angle()
            .approx_eq(other.first_line().angle(), 0.1);
        let mut pts = self.pts;
        if same_line {
            pts.pop();
        }
        pts.extend(other.pts.iter().skip(1));
        PolyLine::new(pts)
    }

    // One or both args might be empty.
    pub fn append(first: Vec<Pt2D>, second: Vec<Pt2D>) -> Vec<Pt2D> {
        if second.is_empty() {
            return first;
        }
        if first.is_empty() {
            return second;
        }

        PolyLine::new(first)
            .extend(&PolyLine::new(second))
            .points()
            .clone()
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

    pub fn length(&self) -> Distance {
        self.length
    }

    // Returns the excess distance left over from the end. None if the result would be too squished
    // together.
    pub fn slice(&self, start: Distance, end: Distance) -> Option<(PolyLine, Distance)> {
        if start > end || start < Distance::ZERO || end < Distance::ZERO {
            panic!("Can't get a polyline slice [{}, {}]", start, end);
        }
        if start > self.length() {
            panic!(
                "Can't get a polyline slice [{}, {}] on something of length {}",
                start,
                end,
                self.length()
            );
        }
        if end - start < EPSILON_DIST {
            return None;
        }

        let mut result: Vec<Pt2D> = Vec::new();
        let mut dist_so_far = Distance::ZERO;

        for line in self.lines().iter() {
            let length = line.length();

            // Does this line contain the first point of the slice?
            if result.is_empty() && dist_so_far + length >= start {
                result.push(line.dist_along(start - dist_so_far));
            }

            // Does this line contain the last point of the slice?
            if dist_so_far + length >= end {
                let last_pt = line.dist_along(end - dist_so_far);
                if result.last().unwrap().epsilon_eq(last_pt) {
                    result.pop();
                }
                result.push(last_pt);
                if result.len() == 1 {
                    // TODO Understand what happened here.
                    return None;
                }
                return Some((PolyLine::new(result), Distance::ZERO));
            }

            // If we're in the middle, just collect the endpoint. But not if it's too close to the
            // previous point (namely, the start, which could be somewhere far along a line)
            if !result.is_empty() && !result.last().unwrap().epsilon_eq(line.pt2()) {
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
        if result.len() == 1 {
            return None;
        }

        Some((PolyLine::new(result), end - dist_so_far))
    }

    pub fn second_half(&self) -> PolyLine {
        self.slice(self.length() / 2.0, self.length()).unwrap().0
    }

    // TODO return result with an error message
    pub fn safe_dist_along(&self, dist_along: Distance) -> Option<(Pt2D, Angle)> {
        if dist_along < Distance::ZERO || dist_along > self.length() {
            return None;
        }

        let mut dist_left = dist_along;
        let mut length_remeasured = Distance::ZERO;
        for (idx, l) in self.lines().iter().enumerate() {
            let length = l.length();
            length_remeasured += length;
            let epsilon = if idx == self.pts.len() - 2 {
                EPSILON_DIST
            } else {
                Distance::ZERO
            };
            if dist_left <= length + epsilon {
                return Some((l.dist_along(dist_left), l.angle()));
            }
            dist_left -= length;
        }
        panic!(
            "PolyLine dist_along of {} broke on length {} (recalculated length {}): {}",
            dist_along,
            self.length(),
            length_remeasured,
            self
        );
    }

    pub fn middle(&self) -> Pt2D {
        self.safe_dist_along(self.length() / 2.0).unwrap().0
    }

    // TODO rm this one
    pub fn dist_along(&self, dist_along: Distance) -> (Pt2D, Angle) {
        if let Some(pair) = self.safe_dist_along(dist_along) {
            return pair;
        }
        if dist_along < Distance::ZERO {
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
    pub fn without_last_line(&self) -> Option<PolyLine> {
        if self.pts.len() == 2 {
            return None;
        }
        Some(PolyLine::new(self.pts[0..self.pts.len() - 1].to_vec()))
    }

    pub fn shift_right(&self, width: Distance) -> PolyLine {
        self.shift_with_corrections(width)
    }

    pub fn shift_left(&self, width: Distance) -> PolyLine {
        self.shift_with_corrections(-width)
    }

    // Things to remember about shifting polylines:
    // - the length before and after probably don't match up
    // - the number of points will match
    fn shift_with_corrections(&self, width: Distance) -> PolyLine {
        let result = PolyLine::new(Pt2D::approx_dedupe(
            self.shift_with_sharp_angles(width),
            EPSILON_DIST,
        ));
        let fixed = if result.pts.len() == self.pts.len() {
            fix_angles(self, result)
        } else {
            result
        };
        // TODO The warning is very spammy. Known issue, silence for now.
        if false {
            check_angles(self, &fixed);
        }
        fixed
    }

    fn shift_with_sharp_angles(&self, width: Distance) -> Vec<Pt2D> {
        if self.pts.len() == 2 {
            let l = Line::new(self.pts[0], self.pts[1]).shift_either_direction(width);
            return vec![l.pt1(), l.pt2()];
        }

        let mut result: Vec<Pt2D> = Vec::new();

        let mut pt3_idx = 2;
        let mut pt1_raw = self.pts[0];
        let mut pt2_raw = self.pts[1];

        loop {
            let pt3_raw = self.pts[pt3_idx];

            let l1 = Line::new(pt1_raw, pt2_raw).shift_either_direction(width);
            let l2 = Line::new(pt2_raw, pt3_raw).shift_either_direction(width);

            if pt3_idx == 2 {
                result.push(l1.pt1());
            }

            if let Some(pt2_shift) = l1.infinite().intersection(&l2.infinite()) {
                result.push(pt2_shift);
            } else {
                // When the lines are perfectly parallel, it means pt2_shift_1st == pt2_shift_2nd
                // and the original geometry is redundant.
                result.push(l1.pt2());
            }
            if pt3_idx == self.pts.len() - 1 {
                result.push(l2.pt2());
                break;
            }

            pt1_raw = pt2_raw;
            pt2_raw = pt3_raw;
            pt3_idx += 1;
        }

        assert!(result.len() == self.pts.len());
        result
    }

    pub fn make_polygons(&self, width: Distance) -> Polygon {
        // TODO Don't use the angle corrections yet -- they seem to do weird things.
        let side1 = self.shift_with_sharp_angles(width / 2.0);
        let side2 = self.shift_with_sharp_angles(-width / 2.0);
        assert_eq!(side1.len(), side2.len());

        let side2_offset = side1.len();
        let mut points = side1;
        points.extend(side2);
        let mut indices = Vec::new();

        for high_idx in 1..self.pts.len() {
            // Duplicate first point, since that's what graphics layer expects
            indices.extend(vec![high_idx, high_idx - 1, side2_offset + high_idx - 1]);
            indices.extend(vec![
                side2_offset + high_idx,
                side2_offset + high_idx - 1,
                high_idx,
            ]);
        }
        Polygon::precomputed(points, indices)
    }

    pub fn dashed_polygons(
        &self,
        width: Distance,
        dash_len: Distance,
        dash_separation: Distance,
    ) -> Vec<Polygon> {
        let mut polygons: Vec<Polygon> = Vec::new();

        let total_length = self.length();

        let mut start = Distance::ZERO;
        loop {
            if start + dash_len >= total_length {
                break;
            }

            polygons.push(
                self.slice(start, start + dash_len)
                    .unwrap()
                    .0
                    .make_polygons(width),
            );
            start += dash_len + dash_separation;
        }

        polygons
    }

    // Also return the angle of the line where the hit was found
    // TODO Also return distance along self of the hit
    pub fn intersection(&self, other: &PolyLine) -> Option<(Pt2D, Angle)> {
        assert_ne!(self, other);

        // There could be several collisions. Pick the "first" from self's perspective.
        for l1 in self.lines() {
            let mut hits: Vec<(Pt2D, Angle)> = Vec::new();
            for l2 in other.lines() {
                if let Some(pt) = l1.intersection(&l2) {
                    hits.push((pt, l1.angle()));
                }
            }

            if let Some(hit) = hits.into_iter().min_by_key(|(pt, _)| {
                self.get_slice_ending_at(*pt)
                    .map(|pl| pl.length())
                    .unwrap_or(Distance::ZERO)
            }) {
                return Some(hit);
            }
        }
        None
    }

    // TODO Also distance along
    pub fn intersection_infinite(&self, other: &InfiniteLine) -> Option<Pt2D> {
        for l in self.lines() {
            if let Some(hit) = l.intersection_infinite(other) {
                return Some(hit);
            }
        }
        None
    }

    // Panics if the pt is not on the polyline. Returns None if the point is the first point
    // (meaning the slice is empty).
    pub fn get_slice_ending_at(&self, pt: Pt2D) -> Option<PolyLine> {
        if self.first_pt() == pt {
            return None;
        }

        if let Some(idx) = self.lines().iter().position(|l| l.contains_pt(pt)) {
            let mut pts = self.pts.clone();
            pts.split_off(idx + 1);
            // Make sure the last line isn't too tiny
            if pts.last().unwrap().epsilon_eq(pt) {
                pts.pop();
            }
            pts.push(pt);
            if pts.len() == 1 {
                return None;
            }
            return Some(PolyLine::new(pts));
        } else {
            panic!("Can't get_slice_ending_at: {} doesn't contain {}", self, pt);
        }
    }

    // Returns None if the point is the last point.
    pub fn get_slice_starting_at(&self, pt: Pt2D) -> Option<PolyLine> {
        if self.last_pt() == pt {
            return None;
        }

        if let Some(idx) = self.lines().iter().position(|l| l.contains_pt(pt)) {
            let mut pts = self.pts.clone();
            pts = pts.split_off(idx + 1);
            pts.insert(0, pt);
            return Some(PolyLine::new(pts));
        } else {
            panic!(
                "Can't get_slice_starting_at: {} doesn't contain {}",
                self, pt
            );
        }
    }

    pub fn dist_along_of_point(&self, pt: Pt2D) -> Option<(Distance, Angle)> {
        let mut dist_along = Distance::ZERO;
        for l in self.lines() {
            if let Some(dist) = l.dist_along_of_point(pt) {
                return Some((dist_along + dist, l.angle()));
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

    // In one side and out another.
    pub fn crosses_polygon(&self, pts: &Vec<Pt2D>) -> bool {
        let mut crossings = 0;
        for l1 in self.lines() {
            for pair in pts.windows(2) {
                if l1.intersection(&Line::new(pair[0], pair[1])).is_some() {
                    crossings += 1;
                }
            }
        }
        if crossings > 2 {
            panic!(
                "{} crosses polygon more than two times! What happened?",
                self
            );
        }
        crossings == 2
    }
}

impl fmt::Display for PolyLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "PolyLine::new(vec![")?;
        for (idx, pt) in self.pts.iter().enumerate() {
            write!(f, "  Pt2D::new({}, {}),", pt.x(), pt.y())?;
            if idx > 0 {
                let line = Line::new(self.pts[idx - 1], *pt);
                write!(
                    f,
                    "    // {}, {} (+ {} @ {})",
                    pt.x() - self.pts[idx - 1].x(),
                    pt.y() - self.pts[idx - 1].y(),
                    line.length(),
                    line.angle(),
                )?;
            }
            writeln!(f)?;
        }
        write!(f, "])")
    }
}

fn fix_angles(orig: &PolyLine, result: PolyLine) -> PolyLine {
    let mut pts = result.pts.clone();

    // Check that the angles roughly match up between the original and shifted line
    for (idx, (orig_l, shifted_l)) in orig.lines().iter().zip(result.lines().iter()).enumerate() {
        let orig_angle = orig_l.angle();
        let shifted_angle = shifted_l.angle();

        if !orig_angle.approx_eq(shifted_angle, 1.0) {
            // When this happens, the rotation is usually right around 180 -- so try swapping
            // the points!
            /*println!(
                "Points changed angles from {} to {} (rot {})",
                orig_angle, shifted_angle, rot
            );*/
            pts.swap(idx, idx + 1);
            // TODO Start the fixing over. but make sure we won't infinite loop...
            //return fix_angles(orig, result);
        }
    }

    // When we swap points, length of the entire PolyLine may change! Recalculating is vital.
    PolyLine::new(pts)
}

fn check_angles(a: &PolyLine, b: &PolyLine) {
    for (orig_l, shifted_l) in a.lines().iter().zip(b.lines().iter()) {
        let orig_angle = orig_l.angle();
        let shifted_angle = shifted_l.angle();

        if !orig_angle.approx_eq(shifted_angle, 1.0) {
            println!(
                "BAD! Points changed angles from {} to {}",
                orig_angle, shifted_angle
            );
        }
    }
}
