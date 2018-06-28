use Line;
use Pt2D;
use graphics::math::Vec2d;
use std::f64;

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

// TODO why do we need a bunch of triangles? why doesn't the single polygon triangulate correctly?
// TODO ideally, detect when the polygon overlaps itself due to sharp lines and too much width
// return Vec2d since this is only used for drawing right now
pub fn polygons_for_polyline(center_pts: &Vec<Pt2D>, width: f64) -> Vec<Vec<Vec2d>> {
    let side1 = shift_polyline(width / 2.0, center_pts);
    let mut reversed_center_pts = center_pts.clone();
    reversed_center_pts.reverse();
    let mut side2 = shift_polyline(width / 2.0, &reversed_center_pts);
    side2.reverse();

    let mut result: Vec<Vec<Pt2D>> = Vec::new();
    for high_idx in 1..center_pts.len() {
        // Duplicate first point, since that's what graphics layer expects
        result.push(vec![
            side1[high_idx],
            side1[high_idx - 1],
            side2[high_idx - 1],
            side1[high_idx],
        ]);
        result.push(vec![
            side2[high_idx],
            side2[high_idx - 1],
            side1[high_idx],
            side2[high_idx],
        ]);
    }
    result
        .iter()
        .map(|pts| pts.iter().map(|pt| pt.to_vec()).collect())
        .collect()
}

pub fn shift_polyline(width: f64, pts: &Vec<Pt2D>) -> Vec<Pt2D> {
    assert!(pts.len() >= 2);
    if pts.len() == 2 {
        let l = Line(pts[0], pts[1]).shift(width);
        return vec![l.pt1(), l.pt2()];
    }

    let mut result: Vec<Pt2D> = Vec::new();

    let mut pt3_idx = 2;
    let mut pt1_raw = pts[0];
    let mut pt2_raw = pts[1];

    loop {
        let pt3_raw = pts[pt3_idx];

        let l1 = Line(pt1_raw, pt2_raw).shift(width);
        let l2 = Line(pt2_raw, pt3_raw).shift(width);
        // When the lines are perfectly parallel, it means pt2_shift_1st == pt2_shift_2nd and the
        // original geometry is redundant.
        let pt2_shift = line_intersection(&l1, &l2).unwrap_or(l1.pt2());

        if pt3_idx == 2 {
            result.push(l1.pt1());
        }
        result.push(pt2_shift);
        if pt3_idx == pts.len() - 1 {
            result.push(l2.pt2());
            break;
        }

        pt1_raw = pt2_raw;
        pt2_raw = pt3_raw;
        pt3_idx += 1;
    }

    assert!(result.len() == pts.len());

    // Check that the angles roughly match up between the original and shifted line
    for (orig_pair, shifted_pair) in pts.windows(2).zip(result.windows(2)) {
        let orig_angle = orig_pair[0].angle_to(orig_pair[1]).normalized_degrees();
        let shifted_angle = shifted_pair[0]
            .angle_to(shifted_pair[1])
            .normalized_degrees();
        let delta = (shifted_angle - orig_angle).abs();
        if delta > 0.00001 {
            println!(
                "Points changed angles from {} to {}",
                orig_angle, shifted_angle
            );
        }
    }

    result
}

// NOT segment. Fails for parallel lines.
// https://en.wikipedia.org/wiki/Line%E2%80%93line_intersection#Given_two_points_on_each_line
pub(crate) fn line_intersection(l1: &Line, l2: &Line) -> Option<Pt2D> {
    let x1 = l1.pt1().x();
    let y1 = l1.pt1().y();
    let x2 = l1.pt2().x();
    let y2 = l1.pt2().y();

    let x3 = l2.pt1().x();
    let y3 = l2.pt1().y();
    let x4 = l2.pt2().x();
    let y4 = l2.pt2().y();

    let numer_x = (x1 * y2 - y1 * x2) * (x3 - x4) - (x1 - x2) * (x3 * y4 - y3 * x4);
    let numer_y = (x1 * y2 - y1 * x2) * (y3 - y4) - (y1 - y2) * (x3 * y4 - y3 * x4);
    let denom = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4);
    if denom == 0.0 {
        None
    } else {
        Some(Pt2D::new(numer_x / denom, numer_y / denom))
    }
}

#[test]
fn shift_polyline_equivalence() {
    use rand;

    let scale = 1000.0;
    let pt1 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);
    let pt2 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);
    let pt3 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);
    let pt4 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);
    let pt5 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);

    let width = 50.0;
    let pt1_s = Line(pt1, pt2).shift(width).pt1();
    let pt2_s =
        line_intersection(&Line(pt1, pt2).shift(width), &Line(pt2, pt3).shift(width)).unwrap();
    let pt3_s =
        line_intersection(&Line(pt2, pt3).shift(width), &Line(pt3, pt4).shift(width)).unwrap();
    let pt4_s =
        line_intersection(&Line(pt3, pt4).shift(width), &Line(pt4, pt5).shift(width)).unwrap();
    let pt5_s = Line(pt4, pt5).shift(width).pt2();

    assert_eq!(
        shift_polyline(width, &vec![pt1, pt2, pt3, pt4, pt5]),
        vec![pt1_s, pt2_s, pt3_s, pt4_s, pt5_s]
    );
}

#[test]
fn shift_short_polyline_equivalence() {
    use rand;

    let scale = 1000.0;
    let pt1 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);
    let pt2 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);

    let width = 50.0;
    let l = Line(pt1, pt2).shift(width);

    assert_eq!(
        shift_polyline(width, &vec![pt1, pt2]),
        vec![l.pt1(), l.pt2()]
    );
}
