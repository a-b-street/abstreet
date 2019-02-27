use crate::runner::TestRunner;
use geom::{Duration, Line, PolyLine, Pt2D};

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

    t.run_fast("time_parsing", |_| {
        assert_eq!(Duration::parse("2.3"), Some(Duration::seconds(2.3)));
        assert_eq!(Duration::parse("02.3"), Some(Duration::seconds(2.3)));
        assert_eq!(Duration::parse("00:00:02.3"), Some(Duration::seconds(2.3)));

        assert_eq!(
            Duration::parse("00:02:03.5"),
            Some(Duration::seconds(123.5))
        );
        assert_eq!(
            Duration::parse("01:02:03.5"),
            Some(Duration::seconds(3723.5))
        );
    });
}

// TODO test that shifting lines and polylines is a reversible operation
