use crate::common::{draw_polyline, BLUE, GREEN, RED};
use ezgui::GfxCtx;
use geom::{Circle, PolyLine, Pt2D};

#[allow(clippy::unreadable_literal)]
pub fn run(g: &mut GfxCtx) {
    let mut vertical_pl = PolyLine::new(vec![
        Pt2D::new(1333.9512635794777, 413.3946082988369),
        Pt2D::new(1333.994382315137, 412.98183477602896),
        Pt2D::new(1334.842742789155, 408.38697863519786),
        Pt2D::new(1341.8334675664184, 388.5049183955915),
        Pt2D::new(1343.4401359706367, 378.05011956849677),
        Pt2D::new(1344.2823018114202, 367.36774792310285),
    ])
    .reversed();
    let mut horiz_pl = PolyLine::new(vec![
        Pt2D::new(1388.995635038006, 411.7906956729764),
        Pt2D::new(1327.388582742321, 410.78740100896965),
    ]);

    let (hit, _) = vertical_pl.intersection(&horiz_pl).unwrap();
    if false {
        g.draw_circle(BLUE, &Circle::new(hit, 1.0));
    } else {
        vertical_pl = vertical_pl.trim_to_pt(hit);
        horiz_pl = horiz_pl.trim_to_pt(hit);
    }

    draw_polyline(g, &vertical_pl, 0.25, RED);
    draw_polyline(g, &horiz_pl, 0.25, GREEN);
}
