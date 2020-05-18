use crate::app::App;
use crate::common::Colorer;
use crate::layer::{Layer, LayerOutcome};
use ezgui::{Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx};
use geom::{ArrowCap, Distance, PolyLine};

pub struct Elevation {
    colorer: Colorer,
    draw: Drawable,
}

impl Layer for Elevation {
    fn name(&self) -> Option<&'static str> {
        Some("elevation")
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        _: &mut App,
        minimap: &Composite,
    ) -> Option<LayerOutcome> {
        self.colorer.legend.align_above(ctx, minimap);
        if self.colorer.event(ctx) {
            return Some(LayerOutcome::Close);
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.colorer.draw(g, app);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.draw);
        }
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.colorer.unzoomed);
        g.redraw(&self.draw);
    }
}

impl Elevation {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Elevation {
        // TODO Two passes because we have to construct the text first :(
        let mut max = 0.0_f64;
        for l in app.primary.map.all_lanes() {
            let pct = l.percent_grade(&app.primary.map).abs();
            max = max.max(pct);
        }

        let mut colorer = Colorer::scaled(
            ctx,
            "Elevation change",
            vec![format!("Steepest road: {:.0}%", max * 100.0)],
            app.cs.good_to_bad.to_vec(),
            vec!["flat", "1%", "5%", "15%", "steeper"],
        );

        let mut max = 0.0_f64;
        for l in app.primary.map.all_lanes() {
            let pct = l.percent_grade(&app.primary.map).abs();
            max = max.max(pct);

            let color = if pct < 0.01 {
                app.cs.good_to_bad[0]
            } else if pct < 0.05 {
                app.cs.good_to_bad[1]
            } else if pct < 0.15 {
                app.cs.good_to_bad[2]
            } else {
                app.cs.good_to_bad[3]
            };
            colorer.add_l(l.id, color, &app.primary.map);
        }

        let arrow_color = Color::BLACK;
        let mut batch = GeomBatch::new();
        // Time for uphill arrows!
        // TODO Draw V's, not arrows.
        // TODO Or try gradient colors.
        for r in app.primary.map.all_roads() {
            let (mut pl, _) = r.get_thick_polyline(&app.primary.map).unwrap();
            let e1 = app.primary.map.get_i(r.src_i).elevation;
            let e2 = app.primary.map.get_i(r.dst_i).elevation;
            if (e1 - e2).abs() / pl.length() < 0.01 {
                // Don't bother with ~flat roads
                continue;
            }
            if e1 > e2 {
                pl = pl.reversed();
            }

            let arrow_len = Distance::meters(5.0);
            let btwn = Distance::meters(10.0);
            let thickness = Distance::meters(1.0);
            let len = pl.length();

            let mut dist = arrow_len;
            while dist + arrow_len <= len {
                let (pt, angle) = pl.dist_along(dist);
                batch.push(
                    arrow_color,
                    PolyLine::new(vec![
                        pt.project_away(arrow_len / 2.0, angle.opposite()),
                        pt.project_away(arrow_len / 2.0, angle),
                    ])
                    .make_arrow(thickness, ArrowCap::Triangle)
                    .unwrap(),
                );
                dist += btwn;
            }
        }

        Elevation {
            colorer: colorer.build_unzoomed(ctx, app),
            draw: batch.upload(ctx),
        }
    }
}
