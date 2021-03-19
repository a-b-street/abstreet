use geom::{ArrowCap, Distance, PolyLine};
use map_gui::tools::{ColorLegend, ColorNetwork};
use map_gui::ID;
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, Panel, Text, TextExt, Widget};

use crate::app::App;
use crate::layer::{header, Layer, LayerOutcome, PANEL_PLACEMENT};

pub struct Elevation {
    tooltip: Option<Text>,
    unzoomed: Drawable,
    zoomed: Drawable,
    panel: Panel,
}

impl Layer for Elevation {
    fn name(&self) -> Option<&'static str> {
        Some("elevation")
    }
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<LayerOutcome> {
        if ctx.redo_mouseover() {
            self.tooltip = None;
            if let Some(ID::Road(r)) = app.mouseover_unzoomed_roads_and_intersections(ctx) {
                self.tooltip = Some(Text::from(Line(format!(
                    "{:.1}% incline",
                    app.primary.map.get_r(r).percent_incline(&app.primary.map).abs() * 100.0
                ))));
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

impl Elevation {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Elevation {
        let mut colorer = ColorNetwork::new(app);

        let mut steepest = 0.0_f64;
        let mut batch = GeomBatch::new();
        for r in app.primary.map.all_roads() {
            let pct = r.percent_incline(&app.primary.map);
            steepest = steepest.max(pct.abs());

            let color = app.cs.good_to_bad_red.eval(
                // Treat 30% as the steepest, rounding off
                (pct.abs() / 0.3).min(1.0),
            );
            colorer.add_r(r.id, color);

            // Draw arrows pointing uphill
            // TODO Draw V's, not arrows.
            // TODO Or try gradient colors.
            if pct.abs() < 0.01 {
                // Don't bother with ~flat roads
                continue;
            }
            let mut pl = r.center_pts.clone();
            if pct < 0.0 {
                pl = pl.reversed();
            }

            let arrow_len = Distance::meters(5.0);
            let btwn = Distance::meters(10.0);
            let thickness = Distance::meters(1.0);
            let len = pl.length();

            let mut dist = arrow_len;
            while dist + arrow_len <= len {
                let (pt, angle) = pl.must_dist_along(dist);
                batch.push(
                    Color::BLACK,
                    PolyLine::must_new(vec![
                        pt.project_away(arrow_len / 2.0, angle.opposite()),
                        pt.project_away(arrow_len / 2.0, angle),
                    ])
                    .make_arrow(thickness, ArrowCap::Triangle),
                );
                dist += btwn;
            }
        }
        colorer.unzoomed.append(batch);

        let panel = Panel::new(Widget::col(vec![
            header(ctx, "Elevation change"),
            format!("Steepest road: {:.0}% incline", steepest * 100.0).text_widget(ctx),
            ColorLegend::gradient(ctx, &app.cs.good_to_bad_red, vec!["flat", "steep"]),
        ]))
        .aligned_pair(PANEL_PLACEMENT)
        .build(ctx);

        Elevation {
            tooltip: None,
            unzoomed: ctx.upload(colorer.unzoomed),
            zoomed: ctx.upload(colorer.zoomed),
            panel,
        }
    }
}
