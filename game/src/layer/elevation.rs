use geom::{ArrowCap, Distance, PolyLine};
use map_gui::tools::{ColorLegend, ColorNetwork};
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, Panel, StyledButtons,
    Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::App;
use crate::layer::{Layer, LayerOutcome};

pub struct Elevation {
    unzoomed: Drawable,
    zoomed: Drawable,
    panel: Panel,
}

impl Layer for Elevation {
    fn name(&self) -> Option<&'static str> {
        Some("elevation")
    }
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App, minimap: &Panel) -> Option<LayerOutcome> {
        Layer::simple_event(ctx, minimap, &mut self.panel)
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.unzoomed);
    }
}

impl Elevation {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Elevation {
        let mut colorer = ColorNetwork::new(app);

        let mut max = 0.0_f64;
        for r in app.primary.map.all_roads() {
            let pct = r.percent_grade(&app.primary.map).abs();
            max = max.max(pct);

            let color = app.cs.good_to_bad_red.eval(
                // TODO Rescale based on a reasonable steepest grade, once the data doesn't suck
                pct.max(0.0).min(1.0),
            );
            colorer.add_r(r.id, color);
        }

        let mut batch = GeomBatch::new();
        // Time for uphill arrows!
        // TODO Draw V's, not arrows.
        // TODO Or try gradient colors.
        for r in app.primary.map.all_roads() {
            let mut pl = r.center_pts.clone();
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
            Widget::row(vec![
                Widget::draw_svg(ctx, "system/assets/tools/layers.svg"),
                "Elevation change".draw_text(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            Text::from_multiline(vec![
                Line(format!("Steepest road: {:.0}% grade", max * 100.0)),
                Line("Note: elevation data is currently wrong!").secondary(),
            ])
            .draw(ctx),
            ColorLegend::gradient(ctx, &app.cs.good_to_bad_red, vec!["flat", "steep"]),
        ]))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        Elevation {
            unzoomed: ctx.upload(colorer.unzoomed),
            zoomed: ctx.upload(colorer.zoomed),
            panel,
        }
    }
}
