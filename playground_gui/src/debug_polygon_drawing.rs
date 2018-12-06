use crate::common::BLUE;
use ezgui::GfxCtx;
use geom::{Polygon, Pt2D};

pub fn run(g: &mut GfxCtx, labels: &mut Vec<(Pt2D, String)>) {
    let pts = vec![
        Pt2D::new(1158.5480421283125, 759.4168710122531), // 0
        Pt2D::new(1158.3757450502824, 776.1517074719404), // 1
        Pt2D::new(1174.6840382119703, 776.3184998618594), // 2
        Pt2D::new(1174.3469352293675, 759.4168710122531), // 3
        Pt2D::new(1158.5480421283125, 759.4168710122531), // 4
    ];
    //draw_polyline(g, &PolyLine::new(pts.clone()), 0.25, RED);
    g.draw_polygon(BLUE, &Polygon::new(&pts));

    for (idx, pt) in pts.iter().enumerate() {
        labels.push((*pt, format!("{}", idx)));
    }
}
