use crate::{
    Angle, Bounds, Distance, HashablePt2D, InfiniteLine, Line, Polygon, Pt2D, Ring, EPSILON_DIST,
};
use abstutil::Warn;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

// TODO How to tune this?
const MITER_THRESHOLD: f64 = 500.0;

pub enum ArrowCap {
    Triangle,
    Lines,
}

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
        if pts.windows(2).any(|pair| pair[0] == pair[1]) {
            panic!(
                "PL with total length {} and {} pts has ~dupe adjacent pts: {:?}",
                length,
                pts.len(),
                pts
            );
        }

        let result = PolyLine { pts, length };

        // Can't have duplicates! If the polyline ever crosses back on itself, all sorts of things
        // are broken.
        let (_, dupes) = to_set(result.points());
        if !dupes.is_empty() {
            panic!(
                "PolyLine has non-adjacent repeat points: {}\nRepeated points: {:?}",
                result, dupes
            );
        }

        result
    }

    pub fn maybe_new(pts: Vec<Pt2D>) -> Option<PolyLine> {
        if pts.len() < 2 {
            return None;
        }
        let length = pts.windows(2).fold(Distance::ZERO, |so_far, pair| {
            so_far + pair[0].dist_to(pair[1])
        });

        if pts.windows(2).any(|pair| pair[0] == pair[1]) {
            return None;
        }

        let result = PolyLine { pts, length };

        let (_, dupes) = to_set(result.points());
        if !dupes.is_empty() {
            return None;
        }

        Some(result)
    }

    // Doesn't check for duplicates. Use at your own risk.
    pub fn unchecked_new(pts: Vec<Pt2D>) -> PolyLine {
        assert!(pts.len() >= 2);
        let length = pts.windows(2).fold(Distance::ZERO, |so_far, pair| {
            so_far + pair[0].dist_to(pair[1])
        });

        PolyLine { pts, length }
    }

    // Only to be called by Ring.
    pub(crate) fn new_for_ring(pts: Vec<Pt2D>) -> PolyLine {
        let length = pts.windows(2).fold(Distance::ZERO, |so_far, pair| {
            so_far + pair[0].dist_to(pair[1])
        });
        PolyLine { pts, length }
    }

    pub fn to_thick_boundary(
        &self,
        self_width: Distance,
        boundary_width: Distance,
    ) -> Option<Polygon> {
        assert!(self_width > boundary_width);
        if self.length() <= boundary_width + EPSILON_DIST {
            return None;
        }
        let slice = self.exact_slice(boundary_width / 2.0, self.length() - boundary_width / 2.0);
        let mut side1 =
            slice.shift_with_sharp_angles((self_width - boundary_width) / 2.0, MITER_THRESHOLD);
        let mut side2 =
            slice.shift_with_sharp_angles(-(self_width - boundary_width) / 2.0, MITER_THRESHOLD);
        side2.reverse();
        side1.extend(side2);
        side1.push(side1[0]);
        side1.dedup();
        Some(Ring::new(side1).make_polygons(boundary_width))
    }

    pub fn reversed(&self) -> PolyLine {
        let mut pts = self.pts.clone();
        pts.reverse();
        PolyLine::new(pts)
    }

    pub fn extend(self, other: PolyLine) -> PolyLine {
        self.maybe_extend(other).unwrap()
    }

    pub fn maybe_extend(self, other: PolyLine) -> Option<PolyLine> {
        assert_eq!(*self.pts.last().unwrap(), other.pts[0]);

        let (pl1, _) = to_set(self.points());
        let (pl2, _) = to_set(&other.points()[1..]);

        let mut self_pts = self.pts;
        let mut other_pts = other.pts;

        if pl1.intersection(&pl2).next().is_some() {
            // Happens on some walking turns. Just clip out the loop. Start searching from the end
            // of 'other'.
            // TODO Measure the length of the thing being clipped out, to be sure this isn't
            // running amok.
            for (other_rev_idx, pt) in other_pts.iter().rev().enumerate() {
                if pl1.contains(&pt.to_hashable()) {
                    while self_pts.last().unwrap() != pt {
                        self_pts.pop();
                    }
                    other_pts = other_pts[other_pts.len() - 1 - other_rev_idx..].to_vec();
                    break;
                }
            }
        }
        // Repeat this for sanity
        assert_eq!(*self_pts.last().unwrap(), other_pts[0]);

        // There's an exciting edge case: the next point to add is on self's last line.
        let same_line = self_pts[self_pts.len() - 2]
            .angle_to(self_pts[self_pts.len() - 1])
            .approx_eq(other_pts[0].angle_to(other_pts[1]), 0.1);
        if same_line {
            self_pts.pop();
        }
        self_pts.extend(other_pts.iter().skip(1));
        PolyLine::maybe_new(self_pts)
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
            .extend(PolyLine::new(second))
            .points()
            .clone()
    }

    pub fn points(&self) -> &Vec<Pt2D> {
        &self.pts
    }
    pub fn into_points(self) -> Vec<Pt2D> {
        self.pts
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
                if *result.last().unwrap() == last_pt {
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
            if !result.is_empty() && *result.last().unwrap() != line.pt2() {
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

    // No excess leftover distance allowed.
    pub fn exact_slice(&self, start: Distance, end: Distance) -> PolyLine {
        let (pl, leftover) = self
            .slice(start, end)
            .unwrap_or_else(|| panic!("exact_slice({}, {}) yielded empty slice", start, end));
        if leftover > EPSILON_DIST {
            panic!(
                "exact_slice({}, {}) on a PL of length {} yielded leftover distance of {}",
                start,
                end,
                self.length(),
                leftover
            );
        }
        pl
    }

    pub fn first_half(&self) -> PolyLine {
        self.exact_slice(Distance::ZERO, self.length() / 2.0)
    }

    pub fn second_half(&self) -> PolyLine {
        self.exact_slice(self.length() / 2.0, self.length())
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

    pub fn shift_right(&self, width: Distance) -> Warn<PolyLine> {
        self.shift_with_corrections(width)
    }

    pub fn shift_left(&self, width: Distance) -> Warn<PolyLine> {
        self.shift_with_corrections(-width)
    }

    // Things to remember about shifting polylines:
    // - the length before and after probably don't match up
    // - the number of points will match
    fn shift_with_corrections(&self, width: Distance) -> Warn<PolyLine> {
        let mut raw = self.shift_with_sharp_angles(width, MITER_THRESHOLD);
        raw.dedup();
        let result = PolyLine::new(raw);
        let fixed = if result.pts.len() == self.pts.len() {
            fix_angles(self, result)
        } else {
            result
        };
        check_angles(self, fixed)
    }

    fn shift_with_sharp_angles(&self, width: Distance, miter_threshold: f64) -> Vec<Pt2D> {
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
                // Miter caps sometimes explode out to infinity. Hackily work around this.
                let dist_away = l1.pt1().raw_dist_to(pt2_shift);
                if dist_away < miter_threshold {
                    result.push(pt2_shift);
                } else {
                    result.push(l1.pt2());
                }
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
        // TODO How to tune this?
        self.make_polygons_with_miter_threshold(width, MITER_THRESHOLD)
    }

    pub fn make_polygons_with_miter_threshold(
        &self,
        width: Distance,
        miter_threshold: f64,
    ) -> Polygon {
        // TODO Don't use the angle corrections yet -- they seem to do weird things.
        let side1 = self.shift_with_sharp_angles(width / 2.0, miter_threshold);
        let side2 = self.shift_with_sharp_angles(-width / 2.0, miter_threshold);
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

    pub fn exact_dashed_polygons(
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
                self.exact_slice(start, start + dash_len)
                    .make_polygons(width),
            );
            start += dash_len + dash_separation;
        }

        polygons
    }

    // Don't draw the dashes too close to the ends.
    pub fn dashed_lines(
        &self,
        width: Distance,
        dash_len: Distance,
        dash_separation: Distance,
    ) -> Vec<Polygon> {
        if self.length() < dash_separation * 2.0 + EPSILON_DIST {
            return vec![self.make_polygons(width)];
        }
        self.exact_slice(dash_separation, self.length() - dash_separation)
            .exact_dashed_polygons(width, dash_len, dash_separation)
    }

    pub fn make_arrow(&self, thickness: Distance, cap: ArrowCap) -> Warn<Polygon> {
        let head_size = thickness * 2.0;
        let triangle_height = head_size / 2.0_f64.sqrt();

        if self.length() < triangle_height + EPSILON_DIST {
            return Warn::warn(
                self.make_polygons(thickness),
                format!("Can't make_arrow of thickness {} for {}", thickness, self),
            );
        }
        let slice = self.exact_slice(Distance::ZERO, self.length() - triangle_height);
        let angle = slice.last_pt().angle_to(self.last_pt());
        let corner1 = self
            .last_pt()
            .project_away(head_size, angle.rotate_degs(-135.0));
        let corner2 = self
            .last_pt()
            .project_away(head_size, angle.rotate_degs(135.0));

        match cap {
            ArrowCap::Triangle => {
                Warn::ok(slice.make_polygons(thickness).union(Polygon::new(&vec![
                    self.last_pt(),
                    corner1,
                    corner2,
                ])))
            }
            ArrowCap::Lines => Warn::ok(self.make_polygons(thickness).union(
                PolyLine::new(vec![corner1, self.last_pt(), corner2]).make_polygons(thickness),
            )),
        }
    }

    // TODO Refactor
    pub fn make_arrow_outline(
        &self,
        arrow_thickness: Distance,
        outline_thickness: Distance,
    ) -> Warn<Vec<Polygon>> {
        let head_size = arrow_thickness * 2.0;
        let triangle_height = head_size / 2.0_f64.sqrt();

        if self.length() < triangle_height {
            return Warn::warn(
                vec![self.make_polygons(arrow_thickness)],
                format!(
                    "Can't make_arrow of thickness {} for {}",
                    arrow_thickness, self
                ),
            );
        }
        let slice = self.exact_slice(Distance::ZERO, self.length() - triangle_height);

        if let Some(p) = slice.to_thick_boundary(arrow_thickness, outline_thickness) {
            let angle = slice.last_pt().angle_to(self.last_pt());
            Warn::ok(vec![
                p,
                Ring::new(vec![
                    self.last_pt(),
                    self.last_pt()
                        .project_away(head_size, angle.rotate_degs(-135.0)),
                    self.last_pt()
                        .project_away(head_size, angle.rotate_degs(135.0)),
                    self.last_pt(),
                ])
                .make_polygons(outline_thickness),
            ])
        } else {
            Warn::warn(
                vec![self.make_polygons(arrow_thickness)],
                format!(
                    "Can't make_arrow_outline of outline_thickness {} for {}",
                    outline_thickness, self
                ),
            )
        }
    }

    pub fn dashed_arrow(
        &self,
        width: Distance,
        dash_len: Distance,
        dash_separation: Distance,
        cap: ArrowCap,
    ) -> Vec<Polygon> {
        let mut polygons = self.exact_dashed_polygons(width, dash_len, dash_separation);
        // And a cap on the arrow. In case the last line is long, trim it to be the dash
        // length.
        let last_line = self.last_line();
        let last_len = last_line.length();
        let arrow_line = if last_len <= dash_len {
            last_line
        } else {
            Line::new(last_line.dist_along(last_len - dash_len), last_line.pt2())
        };
        polygons.push(arrow_line.to_polyline().make_arrow(width, cap).unwrap());
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
            if *pts.last().unwrap() == pt {
                pts.pop();
            }
            pts.push(pt);
            if pts.len() == 1 {
                return None;
            }
            Some(PolyLine::new(pts))
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
            Some(PolyLine::new(pts))
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

    pub fn trim_to_endpts(&self, pt1: Pt2D, pt2: Pt2D) -> PolyLine {
        assert!(pt1 != pt2);
        let mut dist1 = self.dist_along_of_point(pt1).unwrap().0;
        let mut dist2 = self.dist_along_of_point(pt2).unwrap().0;
        if dist1 > dist2 {
            std::mem::swap(&mut dist1, &mut dist2);
        }
        self.exact_slice(dist1, dist2)
    }

    pub fn get_bounds(&self) -> Bounds {
        Bounds::from(&self.pts)
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

fn check_angles(orig: &PolyLine, fixed: PolyLine) -> Warn<PolyLine> {
    let mut warnings = Vec::new();
    for (orig_l, shifted_l) in orig.lines().iter().zip(fixed.lines().iter()) {
        let orig_angle = orig_l.angle();
        let shifted_angle = shifted_l.angle();

        if !orig_angle.approx_eq(shifted_angle, 1.0) {
            warnings.push(format!(
                "Points changed angles from {} to {} during polyline shifting",
                orig_angle, shifted_angle
            ));
        }
    }
    Warn::warnings(fixed, warnings)
}

// Also returns the duplicates.
fn to_set(pts: &[Pt2D]) -> (HashSet<HashablePt2D>, HashSet<HashablePt2D>) {
    let mut deduped = HashSet::new();
    let mut dupes = HashSet::new();
    for pt in pts {
        let pt = pt.to_hashable();
        if deduped.contains(&pt) {
            dupes.insert(pt);
        } else {
            deduped.insert(pt);
        }
    }
    (deduped, dupes)
}
