use geom::{ArrowCap, Distance, PolyLine};
use map_gui::tools::ColorDiscrete;
use map_gui::ID;
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, Panel, Text, TextExt, Widget};

use crate::app::App;
use crate::layer::{header, Layer, LayerOutcome, PANEL_PLACEMENT};

pub struct SteepStreets {
    tooltip: Option<Text>,
    unzoomed: Drawable,
    zoomed: Drawable,
    panel: Panel,
}

impl Layer for SteepStreets {
    fn name(&self) -> Option<&'static str> {
        Some("steep streets")
    }
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<LayerOutcome> {
        if ctx.redo_mouseover() {
            self.tooltip = None;
            if let Some(ID::Road(r)) = app.mouseover_unzoomed_roads_and_intersections(ctx) {
                self.tooltip = Some(Text::from(format!(
                    "{:.1}% incline",
                    app.primary.map.get_r(r).percent_incline.abs() * 100.0
                )));
            }
        }

        Layer::simple_event(ctx, &mut self.panel)
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
        if let Some(ref txt) = self.tooltip {
            g.draw_mouse_tooltip(txt.clone());
        }
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.unzoomed);
    }
}

impl SteepStreets {
    pub fn new(ctx: &mut EventCtx, app: &App) -> SteepStreets {
        let mut colorer = ColorDiscrete::new(
            app,
            vec![
                // Colors and buckets from https://github.com/ITSLeeds/slopes
                ("3-5% (almost flat)", Color::hex("#689A03")),
                ("5-8%", Color::hex("#EB9A04")),
                ("8-10%", Color::hex("#D30800")),
                ("10-20%", Color::hex("#980104")),
                (">20% (steep)", Color::hex("#680605")),
            ],
        );

        let mut steepest = 0.0_f64;
        let mut arrows = GeomBatch::new();
        for r in app.primary.map.all_roads() {
            let pct = r.percent_incline.abs();
            steepest = steepest.max(pct);

            let bucket = if pct < 0.03 {
                continue;
            } else if pct < 0.05 {
                "3-5% (almost flat)"
            } else if pct < 0.08 {
                "5-8%"
            } else if pct < 0.1 {
                "8-10%"
            } else if pct < 0.2 {
                "10-20%"
            } else {
                ">20% (steep)"
            };
            colorer.add_r(r.id, bucket);

            // Draw arrows pointing uphill
            // TODO Draw V's, not arrows.
            // TODO Or try gradient colors.
            let mut pl = r.center_pts.clone();
            if r.percent_incline < 0.0 {
                pl = pl.reversed();
            }

            let arrow_len = Distance::meters(5.0);
            let btwn = Distance::meters(10.0);
            let thickness = Distance::meters(1.0);
            let len = pl.length();

            let mut dist = arrow_len;
            while dist + arrow_len <= len {
                let (pt, angle) = pl.must_dist_along(dist);
                arrows.push(
                    Color::WHITE,
                    PolyLine::must_new(vec![
                        pt.project_away(arrow_len / 2.0, angle.opposite()),
                        pt.project_away(arrow_len / 2.0, angle),
                    ])
                    .make_arrow(thickness, ArrowCap::Triangle),
                );
                dist += btwn;
            }
        }
        colorer.unzoomed.append(arrows);
        let (unzoomed, zoomed, legend) = colorer.build(ctx);

        let panel = Panel::new(Widget::col(vec![
            header(ctx, "Steep streets"),
            format!("Steepest road: {:.0}% incline", steepest * 100.0).text_widget(ctx),
            "Arrows point uphill".text_widget(ctx),
            legend,
        ]))
        .aligned_pair(PANEL_PLACEMENT)
        .build(ctx);

        SteepStreets {
            tooltip: None,
            unzoomed,
            zoomed,
            panel,
        }
    }
}
