use std::collections::HashSet;

use abstutil::prettyprint_usize;
use geom::{Circle, Distance, Pt2D, Time};
use map_gui::tools::{make_heatmap, HeatmapOptions};
use sim::PersonState;
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, Image, Line, Outcome, Panel, Toggle, Widget,
};

use crate::app::App;
use crate::layer::{header, Layer, LayerOutcome, PANEL_PLACEMENT};

// TODO Disable drawing unzoomed agents... or alternatively, implement this by asking Sim to
// return this kind of data instead!
pub struct PopulationMap {
    time: Time,
    opts: Options,
    draw: Drawable,
    panel: Panel,
}

impl Layer for PopulationMap {
    fn name(&self) -> Option<&'static str> {
        Some("population map")
    }
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            let mut new = PopulationMap::new(ctx, app, self.opts.clone());
            new.panel.restore(ctx, &self.panel);
            *self = new;
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Some(LayerOutcome::Close);
                }
                _ => unreachable!(),
            },
            _ => {
                let new_opts = self.options();
                if self.opts != new_opts {
                    *self = PopulationMap::new(ctx, app, new_opts);
                }
            }
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.draw);
        }
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.draw);
    }
}

impl PopulationMap {
    pub fn new(ctx: &mut EventCtx, app: &App, opts: Options) -> PopulationMap {
        let mut pts = Vec::new();
        // Faster to grab all agent positions than individually map trips to agent positions.
        for a in app
            .primary
            .sim
            .get_unzoomed_agents(&app.primary.map)
            .into_iter()
            .chain(
                app.primary
                    .sim
                    .get_unzoomed_transit_riders(&app.primary.map),
            )
        {
            if a.person.is_some() {
                pts.push(a.pos);
            }
        }

        // Many people are probably in the same building. If we're building a heatmap, we
        // absolutely care about these repeats! If we're just drawing the simple dot map, avoid
        // drawing repeat circles.
        let mut seen_bldgs = HashSet::new();
        let mut repeat_pts = Vec::new();
        for person in app.primary.sim.get_all_people() {
            match person.state {
                // Already covered above
                PersonState::Trip(_) => {}
                PersonState::Inside(b) => {
                    let pt = app.primary.map.get_b(b).polygon.center();
                    if seen_bldgs.contains(&b) {
                        repeat_pts.push(pt);
                    } else {
                        seen_bldgs.insert(b);
                        pts.push(pt);
                    }
                }
                PersonState::OffMap => {}
            }
        }

        let mut batch = GeomBatch::new();
        let legend = if let Some(ref o) = opts.heatmap {
            pts.extend(repeat_pts);
            Some(make_heatmap(
                ctx,
                &mut batch,
                app.primary.map.get_bounds(),
                pts,
                o,
            ))
        } else {
            // It's quite silly to produce triangles for the same circle over and over again. ;)
            let circle = Circle::new(Pt2D::new(0.0, 0.0), Distance::meters(10.0)).to_polygon();
            for pt in pts {
                batch.push(Color::RED.alpha(0.8), circle.translate(pt.x(), pt.y()));
            }
            None
        };
        let controls = make_controls(ctx, app, &opts, legend);
        PopulationMap {
            time: app.primary.sim.time(),
            opts,
            draw: ctx.upload(batch),
            panel: controls,
        }
    }

    fn options(&self) -> Options {
        let heatmap = if self.panel.is_checked("Show heatmap") {
            Some(HeatmapOptions::from_controls(&self.panel))
        } else {
            None
        };
        Options { heatmap }
    }
}

#[derive(Clone, PartialEq)]
pub struct Options {
    // If None, just a dot map
    pub heatmap: Option<HeatmapOptions>,
}

fn make_controls(ctx: &mut EventCtx, app: &App, opts: &Options, legend: Option<Widget>) -> Panel {
    let (total_ppl, ppl_in_bldg, ppl_off_map) = app.primary.sim.num_ppl();

    let mut col = vec![
        header(
            ctx,
            &format!("Population: {}", prettyprint_usize(total_ppl)),
        ),
        Widget::row(vec![
            Widget::row(vec![
                Image::icon("system/assets/tools/home.svg").into_widget(ctx),
                Line(prettyprint_usize(ppl_in_bldg))
                    .small()
                    .into_widget(ctx),
            ]),
            Line(format!("Off-map: {}", prettyprint_usize(ppl_off_map)))
                .small()
                .into_widget(ctx),
        ])
        .centered(),
    ];

    col.push(Toggle::switch(
        ctx,
        "Show heatmap",
        None,
        opts.heatmap.is_some(),
    ));
    if let Some(ref o) = opts.heatmap {
        col.extend(o.to_controls(ctx, legend.unwrap()));
    }

    Panel::new(Widget::col(col))
        .aligned_pair(PANEL_PLACEMENT)
        .build(ctx)
}
