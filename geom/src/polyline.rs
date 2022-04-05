use std::collections::HashSet;
use std::fmt;

use anyhow::{Context, Result};
use geo::prelude::ClosestPoint;
use serde::{Deserialize, Serialize};

use crate::{
    Angle, Bounds, Circle, Distance, GPSBounds, HashablePt2D, InfiniteLine, Line, LonLat, Polygon,
    Pt2D, Ring, EPSILON_DIST,
};

// TODO How to tune this?
const MITER_THRESHOLD: f64 = 500.0;

// TODO There used to be a second style that just has extra little hooks going out
pub enum ArrowCap {
    Triangle,
}

// TODO Document and enforce invariants:
// - at least two points
// - no duplicate points, whether adjacent or loops
// - no "useless" intermediate points with the same angle
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PolyLine {
    pts: Vec<Pt2D>,
    // TODO Note that caching length doesn't improve profiling results (by running
    // small_spawn_completes test in release mode). May not be worth doing this.
    length: Distance,
}

impl PolyLine {
    pub fn new(pts: Vec<Pt2D>) -> Result<PolyLine> {
        if pts.len() < 2 {
            bail!("Need at least two points for a PolyLine");
        }
        let length = pts.windows(2).fold(Distance::ZERO, |so_far, pair| {
            so_far + pair[0].dist_to(pair[1])
        });

        if pts.windows(2).any(|pair| pair[0] == pair[1]) {
            bail!(
                "PL with total length {} and {} pts has ~dupe adjacent pts",
                length,
                pts.len(),
            );
        }

        let result = PolyLine { pts, length };

        // Can't have duplicates! If the polyline ever crosses back on itself, all sorts of things
        // are broken.
        let (_, dupes) = to_set(result.points());
        if !dupes.is_empty() {
            bail!(
                "PL with total length {} and {} pts has dupe non-adjacent pts",
                result.length,
                result.pts.len(),
            );
        }

        Ok(result)
    }
    pub fn must_new(pts: Vec<Pt2D>) -> PolyLine {
        PolyLine::new(pts).unwrap()
    }

    /// Doesn't check for duplicates. Use at your own risk.
    pub fn unchecked_new(pts: Vec<Pt2D>) -> PolyLine {
        assert!(pts.len() >= 2);
        let length = pts.windows(2).fold(Distance::ZERO, |so_far, pair| {
            so_far + pair[0].dist_to(pair[1])
        });

        PolyLine { pts, length }
    }

    /// First dedupes adjacent points
    pub fn deduping_new(mut pts: Vec<Pt2D>) -> Result<PolyLine> {
        pts.dedup();
        PolyLine::new(pts)
    }

    /// Like make_polygons, but make sure the points actually form a ring.
    pub fn to_thick_ring(&self, width: Distance) -> Ring {
        let mut side1 = self
            .shift_with_sharp_angles(width / 2.0, MITER_THRESHOLD)
            .unwrap();
        let mut side2 = self
            .shift_with_sharp_angles(-width / 2.0, MITER_THRESHOLD)
            .unwrap();
        side2.reverse();
        side1.extend(side2);
        side1.push(side1[0]);
        side1.dedup();
        Ring::must_new(side1)
    }

    pub fn to_thick_boundary(
        &self,
        self_width: Distance,
        boundary_width: Distance,
    ) -> Option<Polygon> {
        if self_width <= boundary_width || self.length() <= boundary_width + EPSILON_DIST {
            return None;
        }
        // TODO exact_slice() used to work fine here, but the SUMO montlake map triggers a problem
        // there
        let slice = self
            .maybe_exact_slice(boundary_width / 2.0, self.length() - boundary_width / 2.0)
            .ok()?;
        Some(
            slice
                .to_thick_ring(self_width - boundary_width)
                .to_outline(boundary_width),
        )
    }

    pub fn reversed(&self) -> PolyLine {
        let mut pts = self.pts.clone();
        pts.reverse();
        PolyLine::must_new(pts)
    }

    /// Returns the quadrant where the overall angle of this polyline (pointing from the first to
    /// last point) is in. Output between 0 and 3.
    pub fn quadrant(&self) -> i64 {
        let line_angle: f64 = self.overall_angle().normalized_radians();
        let line_angle = (line_angle / (std::f64::consts::PI / 2.0)) as i64;
        line_angle.rem_euclid(4) + 1
    }

    /// Glue together two polylines in order. The last point of `self` must be the same as the
    /// first point of `other`. This method handles removing unnecessary intermediate points if the
    /// extension happens to be at the same angle as the last line segment of `self`.
    pub fn extend(self, other: PolyLine) -> Result<PolyLine> {
        if *self.pts.last().unwrap() != other.pts[0] {
            bail!("can't extend PL; last and first points don't match");
        }

        let mut self_pts = self.pts;
        let mut other_pts = other.pts;

        loop {
            let (pl1, _) = to_set(&self_pts);
            let (pl2, _) = to_set(&other_pts[1..]);

            if pl1.intersection(&pl2).next().is_some() {
                // Happens on some walking turns. Just clip out the loop. Start searching from the
                // end of 'other'.
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
                // Sanity check
                assert_eq!(*self_pts.last().unwrap(), other_pts[0]);
            } else {
                break;
            }
        }

        // There's an exciting edge case: the next point to add is on self's last line.
        if other_pts.len() >= 2 {
            let same_line = self_pts[self_pts.len() - 2]
                .angle_to(self_pts[self_pts.len() - 1])
                .approx_eq(other_pts[0].angle_to(other_pts[1]), 0.1);
            if same_line {
                self_pts.pop();
            }
        }
        self_pts.extend(other_pts.iter().skip(1));
        PolyLine::new(self_pts)
    }

    /// Like `extend`, but panics on failure.
    pub fn must_extend(self, other: PolyLine) -> PolyLine {
        self.extend(other).unwrap()
    }

    /// Extends `self` by a single point. If the new point is close enough to the last, dedupes.
    /// Doesn't clean up any intermediate points.
    pub fn optionally_push(self, pt: Pt2D) -> PolyLine {
        let mut pts = self.into_points();
        pts.push(pt);
        PolyLine::deduping_new(pts).unwrap()
    }

    /// Like `extend`, but handles the last and first point not matching by inserting that point.
    /// Doesn't clean up any intermediate points.
    pub fn force_extend(mut self, other: PolyLine) -> Result<PolyLine> {
        if *self.pts.last().unwrap() != other.pts[0] {
            // TODO Blindly... what if we need to do the angle collapsing?
            self.pts.push(other.pts[0]);
        }
        self.extend(other)
    }

    /// One or both args might be empty.
    pub fn append(first: Vec<Pt2D>, second: Vec<Pt2D>) -> Result<Vec<Pt2D>> {
        if second.is_empty() {
            return Ok(first);
        }
        if first.is_empty() {
            return Ok(second);
        }

        Ok(PolyLine::new(first)?
            .extend(PolyLine::new(second)?)?
            .into_points())
    }

    pub fn points(&self) -> &Vec<Pt2D> {
        &self.pts
    }
    pub fn into_points(self) -> Vec<Pt2D> {
        self.pts
    }

    pub fn lines(&self) -> impl Iterator<Item = Line> + '_ {
        self.pts
            .windows(2)
            .map(|pair| Line::must_new(pair[0], pair[1]))
    }

    pub fn length(&self) -> Distance {
        self.length
    }

    /// Returns the excess distance left over from the end
    pub fn slice(&self, start: Distance, end: Distance) -> Result<(PolyLine, Distance)> {
        if start > end || start < Distance::ZERO || end < Distance::ZERO {
            bail!("Can't get a polyline slice [{}, {}]", start, end);
        }
        if start > self.length() {
            bail!(
                "Can't get a polyline slice [{}, {}] on something of length {}",
                start,
                end,
                self.length()
            );
        }
        if end - start < EPSILON_DIST {
            bail!(
                "Can't get a polyline slice [{}, {}] -- too small",
                start,
                end
            );
        }

        let mut result: Vec<Pt2D> = Vec::new();
        let mut dist_so_far = Distance::ZERO;

        for line in self.lines() {
            let length = line.length();

            // Does this line contain the first point of the slice?
            if result.is_empty() && dist_so_far + length >= start {
                result.push(line.must_dist_along(start - dist_so_far));
            }

            // Does this line contain the last point of the slice?
            if dist_so_far + length >= end {
                let last_pt = line.must_dist_along(end - dist_so_far);
                if *result.last().unwrap() == last_pt {
                    result.pop();
                }
                result.push(last_pt);
                if result.len() == 1 {
                    // TODO Understand what happened here.
                    bail!("slice({}, {}) on {} did something weird", start, end, self);
                }
                return Ok((PolyLine::new(result)?, Distance::ZERO));
            }

            // If we're in the middle, just collect the endpoint. But not if it's too close to the
            // previous point (namely, the start, which could be somewhere far along a line)
            if !result.is_empty() && *result.last().unwrap() != line.pt2() {
                result.push(line.pt2());
            }

            dist_so_far += length;
        }

        if result.is_empty() {
            bail!(
                "Slice [{}, {}] has a start too big for polyline of length {}",
                start,
                end,
                self.length()
            );
        }
        if result.len() == 1 {
            bail!(
                "Slice [{}, {}] on {} wound up a single point",
                start,
                end,
                self
            );
        }

        Ok((PolyLine::new(result)?, end - dist_so_far))
    }

    /// No excess leftover distance allowed.
    // TODO Lot of callers of this. Make safer later.
    pub fn exact_slice(&self, start: Distance, end: Distance) -> PolyLine {
        self.maybe_exact_slice(start, end).unwrap()
    }
    pub fn maybe_exact_slice(&self, start: Distance, end: Distance) -> Result<PolyLine> {
        let (pl, leftover) = self
            .slice(start, end)
            .with_context(|| format!("exact_slice({}, {}) yielded empty slice", start, end))?;
        if leftover > EPSILON_DIST {
            bail!(
                "exact_slice({}, {}) on a PL of length {} yielded leftover distance of {}",
                start,
                end,
                self.length(),
                leftover
            );
        }
        Ok(pl)
    }

    pub fn first_half(&self) -> PolyLine {
        self.exact_slice(Distance::ZERO, self.length() / 2.0)
    }

    pub fn second_half(&self) -> PolyLine {
        self.exact_slice(self.length() / 2.0, self.length())
    }

    pub fn dist_along(&self, dist_along: Distance) -> Result<(Pt2D, Angle)> {
        if dist_along < Distance::ZERO {
            bail!("dist_along {} is negative", dist_along);
        }
        if dist_along > self.length() {
            bail!("dist_along {} is longer than {}", dist_along, self.length());
        }
        if dist_along == self.length() {
            return Ok((self.last_pt(), self.last_line().angle()));
        }

        let mut dist_left = dist_along;
        for (idx, l) in self.lines().enumerate() {
            let length = l.length();
            let epsilon = if idx == self.pts.len() - 2 {
                EPSILON_DIST
            } else {
                Distance::ZERO
            };
            if dist_left <= length + epsilon {
                // Floating point errors means sometimes we ask for something slightly longer than
                // the line
                let dist = l.dist_along(dist_left).unwrap_or_else(|_| l.pt2());
                return Ok((dist, l.angle()));
            }
            dist_left -= length;
        }
        // Leaving this panic, because I haven't seen this in ages, and something is seriously
        // wrong if we get here
        panic!(
            "PolyLine dist_along of {} broke on length {}: {}",
            dist_along,
            self.length(),
            self
        );
    }
    pub fn must_dist_along(&self, dist_along: Distance) -> (Pt2D, Angle) {
        self.dist_along(dist_along).unwrap()
    }

    pub fn middle(&self) -> Pt2D {
        // If this fails, must be some super tiny line. Just return the first point in that case.
        match self.dist_along(self.length() / 2.0) {
            Ok((pt, _)) => pt,
            Err(err) => {
                println!(
                    "Guessing middle of PL with length {}: {}",
                    self.length(),
                    err
                );
                self.first_pt()
            }
        }
    }

    pub fn first_pt(&self) -> Pt2D {
        self.pts[0]
    }
    pub fn last_pt(&self) -> Pt2D {
        *self.pts.last().unwrap()
    }
    pub fn first_line(&self) -> Line {
        Line::must_new(self.pts[0], self.pts[1])
    }
    pub fn last_line(&self) -> Line {
        Line::must_new(self.pts[self.pts.len() - 2], self.pts[self.pts.len() - 1])
    }

    pub fn shift_right(&self, width: Distance) -> Result<PolyLine> {
        self.shift_with_corrections(width)
    }
    pub fn must_shift_right(&self, width: Distance) -> PolyLine {
        self.shift_right(width).unwrap()
    }

    pub fn shift_left(&self, width: Distance) -> Result<PolyLine> {
        self.shift_with_corrections(-width)
    }
    pub fn must_shift_left(&self, width: Distance) -> PolyLine {
        self.shift_left(width).unwrap()
    }

    /// Perpendicularly shifts the polyline to the right if positive or left if negative.
    pub fn shift_either_direction(&self, width: Distance) -> Result<PolyLine> {
        self.shift_with_corrections(width)
    }

    /// `self` represents some center, with `total_width`. Logically this shifts left by
    /// `total_width / 2`, then right by `width_from_left_side`, but without exasperating sharp
    /// bends.
    pub fn shift_from_center(
        &self,
        total_width: Distance,
        width_from_left_side: Distance,
    ) -> Result<PolyLine> {
        let half_width = total_width / 2.0;
        if width_from_left_side < half_width {
            self.shift_left(half_width - width_from_left_side)
        } else {
            self.shift_right(width_from_left_side - half_width)
        }
    }

    // Things to remember about shifting polylines:
    // - the length before and after probably don't match up
    // - the number of points may not match
    fn shift_with_corrections(&self, width: Distance) -> Result<PolyLine> {
        let raw = self.shift_with_sharp_angles(width, MITER_THRESHOLD)?;
        let result = PolyLine::deduping_new(raw)?;
        if result.pts.len() == self.pts.len() {
            fix_angles(self, result)
        } else {
            Ok(result)
        }
    }

    // If we start with a valid PolyLine, I'm not sure how we can ever possibly fail here, but it's
    // happening. Avoid crashing.
    fn shift_with_sharp_angles(&self, width: Distance, miter_threshold: f64) -> Result<Vec<Pt2D>> {
        if self.pts.len() == 2 {
            let l = Line::new(self.pts[0], self.pts[1])?.shift_either_direction(width);
            return Ok(vec![l.pt1(), l.pt2()]);
        }

        let mut result: Vec<Pt2D> = Vec::new();

        let mut pt3_idx = 2;
        let mut pt1_raw = self.pts[0];
        let mut pt2_raw = self.pts[1];

        loop {
            let pt3_raw = self.pts[pt3_idx];

            let l1 = Line::new(pt1_raw, pt2_raw)?.shift_either_direction(width);
            let l2 = Line::new(pt2_raw, pt3_raw)?.shift_either_direction(width);

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
        Ok(result)
    }

    /// The resulting polygon is manually triangulated and may not have a valid outer Ring (but it
    /// usually does).
    pub fn make_polygons(&self, width: Distance) -> Polygon {
        // TODO Don't use the angle corrections yet -- they seem to do weird things.
        let side1 = match self.shift_with_sharp_angles(width / 2.0, MITER_THRESHOLD) {
            Ok(pl) => pl,
            Err(err) => {
                // TODO Circles will look extremely bizarre, but it emphasizes there's a bug
                // without just crashing
                println!("make_polygons({}) of {:?} failed: {}", width, self, err);
                return Circle::new(self.first_pt(), width).to_polygon();
            }
        };
        let mut side2 = match self.shift_with_sharp_angles(-width / 2.0, MITER_THRESHOLD) {
            Ok(pl) => pl,
            Err(err) => {
                println!("make_polygons({}) of {:?} failed: {}", width, self, err);
                return Circle::new(self.first_pt(), width).to_polygon();
            }
        };
        assert_eq!(side1.len(), side2.len());

        // Order the points so that they form a ring. No deduplication yet, though.
        let len = 2 * side1.len();
        let mut points = side1;
        side2.reverse();
        points.extend(side2);
        points.push(points[0]);
        let mut indices = Vec::new();

        // Walk along the first side, making two triangles each step. This is easy to understand
        // with a simple diagram, which I should eventually draw in ASCII art here.
        for high_idx in 1..self.pts.len() {
            indices.extend(vec![high_idx, high_idx - 1, len - high_idx]);
            indices.extend(vec![len - high_idx, len - high_idx - 1, high_idx]);
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

    /// Don't draw the dashes too close to the ends.
    pub fn dashed_lines(
        &self,
        width: Distance,
        dash_len: Distance,
        dash_separation: Distance,
    ) -> Vec<Polygon> {
        if self.length() <= dash_separation * 2.0 + EPSILON_DIST {
            return vec![self.make_polygons(width)];
        }
        self.exact_slice(dash_separation, self.length() - dash_separation)
            .exact_dashed_polygons(width, dash_len, dash_separation)
    }

    /// Fail if the length is too short.
    pub fn maybe_make_arrow(&self, thickness: Distance, cap: ArrowCap) -> Option<Polygon> {
        let head_size = thickness * 2.0;
        let triangle_height = head_size / 2.0_f64.sqrt();

        let slice = self
            .maybe_exact_slice(Distance::ZERO, self.length() - triangle_height)
            .ok()?;

        let angle = slice.last_pt().angle_to(self.last_pt());
        let corner1 = self
            .last_pt()
            .project_away(head_size, angle.rotate_degs(-135.0));
        let corner2 = self
            .last_pt()
            .project_away(head_size, angle.rotate_degs(135.0));

        let mut pts = slice
            .shift_with_sharp_angles(thickness / 2.0, MITER_THRESHOLD)
            .ok()?;
        match cap {
            ArrowCap::Triangle => {
                pts.push(corner2);
                pts.push(self.last_pt());
                pts.push(corner1);
            }
        }
        let mut side2 = slice
            .shift_with_sharp_angles(-thickness / 2.0, MITER_THRESHOLD)
            .ok()?;
        side2.reverse();
        pts.extend(side2);
        pts.push(pts[0]);
        pts.dedup();
        Some(Ring::must_new(pts).into_polygon())
    }

    /// If the length is too short, just give up and make the thick line
    pub fn make_arrow(&self, thickness: Distance, cap: ArrowCap) -> Polygon {
        if let Some(p) = self.maybe_make_arrow(thickness, cap) {
            p
        } else {
            // Just give up and make the thick line.
            self.make_polygons(thickness)
        }
    }

    pub fn make_double_arrow(&self, thickness: Distance, cap: ArrowCap) -> Polygon {
        let head_size = thickness * 2.0;
        let triangle_height = head_size / 2.0_f64.sqrt();

        if self.length() < triangle_height * 2.0 + EPSILON_DIST {
            // Just give up and make the thick line.
            return self.make_polygons(thickness);
        }
        let slice = self.exact_slice(triangle_height, self.length() - triangle_height);

        let angle = slice.last_pt().angle_to(self.last_pt());
        let corner1 = self
            .last_pt()
            .project_away(head_size, angle.rotate_degs(-135.0));
        let corner2 = self
            .last_pt()
            .project_away(head_size, angle.rotate_degs(135.0));

        let mut pts = match slice.shift_with_sharp_angles(thickness / 2.0, MITER_THRESHOLD) {
            Ok(pl) => pl,
            Err(_) => {
                return self.make_polygons(thickness);
            }
        };
        match cap {
            ArrowCap::Triangle => {
                pts.push(corner2);
                pts.push(self.last_pt());
                pts.push(corner1);
            }
        }
        let mut side2 = match slice.shift_with_sharp_angles(-thickness / 2.0, MITER_THRESHOLD) {
            Ok(pl) => pl,
            Err(_) => {
                return self.make_polygons(thickness);
            }
        };
        side2.reverse();
        pts.extend(side2);

        let angle = self.first_pt().angle_to(slice.first_pt());
        let corner3 = self
            .first_pt()
            .project_away(head_size, angle.rotate_degs(-45.0));
        let corner4 = self
            .first_pt()
            .project_away(head_size, angle.rotate_degs(45.0));
        match cap {
            ArrowCap::Triangle => {
                pts.push(corner3);
                pts.push(self.first_pt());
                pts.push(corner4);
            }
        }

        pts.push(pts[0]);
        pts.dedup();
        Ring::must_new(pts).into_polygon()
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
            Line::must_new(
                last_line.must_dist_along(last_len - dash_len),
                last_line.pt2(),
            )
        };
        polygons.push(arrow_line.to_polyline().make_arrow(width, cap));
        polygons
    }

    /// Also return the angle of the line where the hit was found
    // TODO Also return distance along self of the hit
    pub fn intersection(&self, other: &PolyLine) -> Option<(Pt2D, Angle)> {
        assert_ne!(self, other);

        // There could be several collisions. Pick the "first" from self's perspective.
        let mut closest_intersection: Option<(Pt2D, Angle)> = None;
        let mut closest_intersection_distance: Option<Distance> = None;

        for l1 in self.lines() {
            for l2 in other.lines() {
                if let Some(pt) = l1.intersection(&l2) {
                    if let Some(new_distance) = self.get_slice_ending_at(pt).map(|pl| pl.length()) {
                        match closest_intersection_distance {
                            None => {
                                closest_intersection = Some((pt, l1.angle()));
                                closest_intersection_distance = Some(new_distance);
                            }
                            Some(existing_distance) if existing_distance > new_distance => {
                                closest_intersection = Some((pt, l1.angle()));
                                closest_intersection_distance = Some(new_distance);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // TODO Why is any of this necessary? Found a test case at the intersection geometry for
        // https://www.openstreetmap.org/node/274088813 where this made a huge difference!
        if closest_intersection.is_none() && self.last_pt() == other.last_pt() {
            return Some((self.last_pt(), self.last_line().angle()));
        }

        closest_intersection
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

    /// Panics if the pt is not on the polyline. Returns None if the point is the first point
    /// (meaning the slice is empty).
    pub fn get_slice_ending_at(&self, pt: Pt2D) -> Option<PolyLine> {
        if self.first_pt() == pt {
            return None;
        }

        if let Some(idx) = self.lines().position(|l| l.contains_pt(pt)) {
            let mut pts = self.pts.clone();
            pts.truncate(idx + 1);
            // Make sure the last line isn't too tiny
            if *pts.last().unwrap() == pt {
                pts.pop();
            }
            pts.push(pt);
            if pts.len() == 1 {
                return None;
            }
            Some(PolyLine::must_new(pts))
        } else {
            panic!("Can't get_slice_ending_at: {} doesn't contain {}", self, pt);
        }
    }

    /// Returns None if the point is the last point.
    pub fn get_slice_starting_at(&self, pt: Pt2D) -> Option<PolyLine> {
        if self.last_pt() == pt {
            return None;
        }

        if let Some(idx) = self.lines().position(|l| l.contains_pt(pt)) {
            let mut pts = self.pts.clone();
            pts = pts.split_off(idx + 1);
            if pt != pts[0] {
                pts.insert(0, pt);
            }
            Some(PolyLine::must_new(pts))
        } else {
            panic!(
                "Can't get_slice_starting_at: {} doesn't contain {}",
                self, pt
            );
        }
    }

    /// Same as get_slice_ending_at, but returns None if the point isn't on the polyline.
    // TODO Switch everything to this, after better understanding why this is happening at all.
    pub fn safe_get_slice_ending_at(&self, pt: Pt2D) -> Option<PolyLine> {
        if self.first_pt() == pt {
            return None;
        }

        if let Some(idx) = self.lines().position(|l| l.contains_pt(pt)) {
            let mut pts = self.pts.clone();
            pts.truncate(idx + 1);
            // Make sure the last line isn't too tiny
            if *pts.last().unwrap() == pt {
                pts.pop();
            }
            pts.push(pt);
            if pts.len() == 1 {
                return None;
            }
            Some(PolyLine::must_new(pts))
        } else {
            None
        }
    }

    /// Same as get_slice_starting_at, but returns None if the point isn't on the polyline.
    pub fn safe_get_slice_starting_at(&self, pt: Pt2D) -> Option<PolyLine> {
        if self.last_pt() == pt {
            return None;
        }

        if let Some(idx) = self.lines().position(|l| l.contains_pt(pt)) {
            let mut pts = self.pts.clone();
            pts = pts.split_off(idx + 1);
            if pt != pts[0] {
                pts.insert(0, pt);
            }
            Some(PolyLine::must_new(pts))
        } else {
            None
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

    /// If the current line is at least this long, return it. Otherwise, extend the end of it,
    /// following the angle of the last line.
    pub fn extend_to_length(&self, min_len: Distance) -> PolyLine {
        let need_len = min_len - self.length();
        if need_len <= Distance::ZERO {
            return self.clone();
        }
        let line = self.last_line();
        // We might be extending a very tiny amount
        if let Ok(extension) = PolyLine::new(vec![
            line.pt2(),
            line.pt2().project_away(need_len, line.angle()),
        ]) {
            self.clone().must_extend(extension)
        } else {
            let mut pts = self.clone().into_points();
            pts.pop();
            pts.push(line.pt2().project_away(need_len, line.angle()));
            PolyLine::must_new(pts)
        }
    }

    /// Produces a GeoJSON linestring, optionally mapping the world-space points back to GPS.
    pub fn to_geojson(&self, gps: Option<&GPSBounds>) -> geojson::Geometry {
        let mut pts = Vec::new();
        if let Some(gps) = gps {
            for pt in gps.convert_back(&self.pts) {
                pts.push(vec![pt.x(), pt.y()]);
            }
        } else {
            for pt in &self.pts {
                pts.push(vec![pt.x(), pt.y()]);
            }
        }
        geojson::Geometry::new(geojson::Value::LineString(pts))
    }

    pub fn from_geojson(feature: &geojson::Feature, gps: Option<&GPSBounds>) -> Result<PolyLine> {
        if let Some(geojson::Geometry {
            value: geojson::Value::LineString(ref pts),
            ..
        }) = feature.geometry
        {
            let mut points = Vec::new();
            for pt in pts {
                let x = pt[0];
                let y = pt[1];
                if let Some(ref gps) = gps {
                    points.push(LonLat::new(x, y).to_pt(gps));
                } else {
                    points.push(Pt2D::new(x, y));
                }
            }
            PolyLine::new(points)
        } else {
            bail!("Input isn't a LineString")
        }
    }

    /// Returns the point on the polyline closest to the query.
    pub fn project_pt(&self, query: Pt2D) -> Pt2D {
        match self
            .to_geo()
            .closest_point(&geo::Point::new(query.x(), query.y()))
        {
            geo::Closest::Intersection(hit) | geo::Closest::SinglePoint(hit) => {
                Pt2D::new(hit.x(), hit.y())
            }
            geo::Closest::Indeterminate => unreachable!(),
        }
    }

    /// Returns the angle from the start to end of this polyline.
    pub fn overall_angle(&self) -> Angle {
        self.first_pt().angle_to(self.last_pt())
    }

    pub(crate) fn to_geo(&self) -> geo::LineString<f64> {
        let pts: Vec<geo::Point<f64>> = self
            .pts
            .iter()
            .map(|pt| geo::Point::new(pt.x(), pt.y()))
            .collect();
        pts.into()
    }

    /// Walk along the PolyLine, starting `buffer_ends` from the start and ending `buffer_ends`
    /// before the end. Advance in increments of `step_size`. Returns the point and angle at each
    /// step.
    pub fn step_along(&self, step_size: Distance, buffer_ends: Distance) -> Vec<(Pt2D, Angle)> {
        self.step_along_start_end(step_size, buffer_ends, buffer_ends)
    }

    /// Walk along the PolyLine, from `start_buffer` to `length - end_buffer`. Advance in
    /// increments of `step_size`. Returns the point and angle at each step.
    pub fn step_along_start_end(
        &self,
        step_size: Distance,
        start_buffer: Distance,
        end_buffer: Distance,
    ) -> Vec<(Pt2D, Angle)> {
        let mut result = Vec::new();
        let mut dist_along = start_buffer;
        let length = self.length();
        while dist_along < length - end_buffer {
            result.push(self.must_dist_along(dist_along));
            dist_along += step_size;
        }
        result
    }

    ///
    /// ```
    /// use geom::{PolyLine, Pt2D, Distance};
    ///
    /// let polyline = PolyLine::must_new(vec![
    ///     Pt2D::new(0.0, 0.0),
    ///     Pt2D::new(0.0, 10.0),
    ///     Pt2D::new(10.0, 20.0),
    /// ]);
    ///
    /// assert_eq!(
    ///     polyline.interpolate_points(Distance::meters(20.0)).points(),
    ///     &vec![
    ///         Pt2D::new(0.0, 0.0),
    ///         Pt2D::new(0.0, 10.0),
    ///         Pt2D::new(10.0, 20.0),
    ///     ]
    /// );
    ///
    /// assert_eq!(
    ///     polyline.interpolate_points(Distance::meters(10.0)).points(),
    ///     &vec![
    ///         Pt2D::new(0.0, 0.0),
    ///         Pt2D::new(0.0, 10.0),
    ///         Pt2D::new(5.0, 15.0),
    ///         Pt2D::new(10.0, 20.0),
    ///     ]
    /// );
    ///
    /// ```
    pub fn interpolate_points(&self, max_step: Distance) -> PolyLine {
        if self.pts.len() < 2 {
            return self.clone();
        }

        let mut output = vec![];
        for line in self.lines() {
            let points = (line.length() / max_step).ceil();
            let step_size = line.length() / points;
            for i in 0..(points as usize) {
                output.push(line.must_dist_along(step_size * i as f64));
            }
        }

        output.push(*self.pts.last().unwrap());

        PolyLine::new(output).unwrap()
    }

    /// An arbitrary placeholder value, when Option types aren't worthwhile
    pub fn dummy() -> PolyLine {
        PolyLine::must_new(vec![Pt2D::new(0.0, 0.0), Pt2D::new(0.1, 0.1)])
    }
}

impl fmt::Display for PolyLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "PolyLine::new(vec![     // length {}", self.length)?;
        for (idx, pt) in self.pts.iter().enumerate() {
            write!(f, "  Pt2D::new({}, {}),", pt.x(), pt.y())?;
            if idx > 0 {
                let line = Line::must_new(self.pts[idx - 1], *pt);
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

fn fix_angles(orig: &PolyLine, result: PolyLine) -> Result<PolyLine> {
    let mut pts = result.pts.clone();

    // Check that the angles roughly match up between the original and shifted line
    for (idx, (orig_l, shifted_l)) in orig.lines().zip(result.lines()).enumerate() {
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
