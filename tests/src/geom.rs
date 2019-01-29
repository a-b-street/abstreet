use crate::runner::TestRunner;
use geom::{Line, PolyLine, Pt2D};
use rand;

#[allow(clippy::unreadable_literal)]
pub fn run(t: &mut TestRunner) {
    t.run_fast("dist_along_horiz_line", |_| {
        let l = Line::new(
            Pt2D::new(147.17832753158294, 1651.034235433578),
            Pt2D::new(185.9754103560146, 1651.0342354335778),
        );
        let pt = Pt2D::new(179.1628455160347, 1651.0342354335778);

        assert!(l.contains_pt(pt));
        assert!(l.dist_along_of_point(pt).is_some());
    });

    t.run_fast("shift_polyline_equivalence", |_| {
        let scale = 1000.0;
        let pt1 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);
        let pt2 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);
        let pt3 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);
        let pt4 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);
        let pt5 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);

        let width = 50.0;
        let pt1_s = Line::new(pt1, pt2).shift_right(width).pt1();
        let pt2_s = Line::new(pt1, pt2)
            .shift_right(width)
            .infinite()
            .intersection(&Line::new(pt2, pt3).shift_right(width).infinite())
            .unwrap();
        let pt3_s = Line::new(pt2, pt3)
            .shift_right(width)
            .infinite()
            .intersection(&Line::new(pt3, pt4).shift_right(width).infinite())
            .unwrap();
        let pt4_s = Line::new(pt3, pt4)
            .shift_right(width)
            .infinite()
            .intersection(&Line::new(pt4, pt5).shift_right(width).infinite())
            .unwrap();
        let pt5_s = Line::new(pt4, pt5).shift_right(width).pt2();

        assert_eq!(
            PolyLine::new(vec![pt1, pt2, pt3, pt4, pt5]).shift_right(width),
            PolyLine::new(vec![pt1_s, pt2_s, pt3_s, pt4_s, pt5_s])
        );
    });

    t.run_fast("shift_short_polyline_equivalence", |_| {
        let scale = 1000.0;
        let pt1 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);
        let pt2 = Pt2D::new(rand::random::<f64>() * scale, rand::random::<f64>() * scale);

        let width = 50.0;
        let l = Line::new(pt1, pt2).shift_right(width);

        assert_eq!(
            PolyLine::new(vec![pt1, pt2]).shift_right(width),
            l.to_polyline()
        );
    });

    t.run_fast("trim_with_epsilon", |_| {
        /*
        // EPSILON_DIST needs to be tuned correctly, or this point seems like it's not on the line.
        let mut pl = PolyLine::new(vec![
          Pt2D::new(1130.2653468611902, 2124.099702776818),
          Pt2D::new(1175.9652436108408, 2124.1094748373457),
          Pt2D::new(1225.8319649025132, 2124.120594334445),
        ]);
        let pt = Pt2D::new(1225.8319721124885, 2124.1205943360505);*/

        let pl = PolyLine::new(vec![
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

        pl.get_slice_ending_at(pt);
    });
}

// TODO test that shifting lines and polylines is a reversible operation
