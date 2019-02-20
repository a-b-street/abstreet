use crate::common::{draw_polyline, BLACK, BLUE, GREEN, RED};
use ezgui::GfxCtx;
use geom::{Distance, PolyLine, Pt2D};

pub fn run(p3_offset: (f64, f64), g: &mut GfxCtx, labels: &mut Vec<(Pt2D, String)>) {
    macro_rules! point {
        ($pt_name:ident, $value:expr) => {
            let $pt_name = $value;
            labels.push(($pt_name, stringify!($pt_name).to_string()));
        };
    }
    /*macro_rules! points {
        ($pt1_name:ident, $pt2_name:ident, $value:expr) => {
            let ($pt1_name, $pt2_name) = $value;
            labels.push(($pt1_name, stringify!($pt1_name).to_string()));
            labels.push(($pt2_name, stringify!($pt2_name).to_string()));
        };
    }*/

    let thin = Distance::meters(1.0);
    let thick = Distance::meters(5.0);
    let shift_away = Distance::meters(50.0);

    point!(p1, Pt2D::new(100.0, 100.0));
    point!(p2, Pt2D::new(110.0, 200.0));
    point!(p3, Pt2D::new(p1.x() + p3_offset.0, p1.y() + p3_offset.1));
    point!(p4, Pt2D::new(500.0, 120.0));

    /*println!();
    println!("p1 -> p2 is {}", p1.angle_to(p2));
    println!("p2 -> p3 is {}", p2.angle_to(p3));*/

    let pts = PolyLine::new(vec![p1, p2, p3, p4]);

    draw_polyline(g, &pts, thick, RED);

    g.draw_polygon(BLACK, &pts.make_polygons(shift_away));

    // Two lanes on one side of the road
    let l1_pts = pts.shift_right(shift_away).unwrap();
    for (idx, pt) in l1_pts.points().iter().enumerate() {
        labels.push((*pt, format!("l1_p{}", idx + 1)));
    }
    draw_polyline(g, &l1_pts, thin, GREEN);

    let l2_pts = pts.shift_right(shift_away * 2.0).unwrap();
    for (idx, pt) in l2_pts.points().iter().enumerate() {
        labels.push((*pt, format!("l2_p{}", idx + 1)));
    }
    draw_polyline(g, &l2_pts, thin, GREEN);

    // Other side
    let l3_pts = pts.reversed().shift_right(shift_away).unwrap();
    for (idx, pt) in l3_pts.points().iter().enumerate() {
        labels.push((*pt, format!("l3_p{}", idx + 1)));
    }
    draw_polyline(g, &l3_pts, thin, BLUE);
}
