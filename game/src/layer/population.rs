use crate::app::App;
use crate::common::{make_heatmap, HeatmapOptions};
use crate::layer::{Layer, LayerOutcome};
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Btn, Checkbox, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, VerticalAlignment, Widget,
};
use geom::{Circle, Distance, Pt2D, Time};
use sim::{GetDrawAgents, PersonState};
use std::collections::HashSet;

// TODO Disable drawing unzoomed agents... or alternatively, implement this by asking Sim to
// return this kind of data instead!
pub struct PopulationMap {
    time: Time,
    opts: Options,
    draw: Drawable,
    composite: Composite,
}

impl Layer for PopulationMap {
    fn name(&self) -> Option<&'static str> {
        Some("population map")
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        minimap: &Composite,
    ) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            *self = PopulationMap::new(ctx, app, self.opts.clone());
        }

        self.composite.align_above(ctx, minimap);
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Some(LayerOutcome::Close);
                }
                _ => unreachable!(),
            },
            None => {
                let new_opts = self.options();
                if self.opts != new_opts {
                    *self = PopulationMap::new(ctx, app, new_opts);
                    self.composite.align_above(ctx, minimap);
                }
            }
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.composite.draw(g);
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
        for a in app.primary.sim.get_unzoomed_agents(&app.primary.map) {
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
        let colors_and_labels = if let Some(ref o) = opts.heatmap {
            pts.extend(repeat_pts);
            Some(make_heatmap(
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
        let controls = make_controls(ctx, app, &opts, colors_and_labels);
        PopulationMap {
            time: app.primary.sim.time(),
            opts,
            draw: ctx.upload(batch),
            composite: controls,
        }
    }

    fn options(&self) -> Options {
        let heatmap = if self.composite.is_checked("Show heatmap") {
            Some(HeatmapOptions::from_controls(&self.composite))
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

fn make_controls(
    ctx: &mut EventCtx,
    app: &App,
    opts: &Options,
    colors_and_labels: Option<(Vec<Color>, Vec<String>)>,
) -> Composite {
    let (total_ppl, ppl_in_bldg, ppl_off_map) = app.primary.sim.num_ppl();

    let mut col = vec![
        Widget::row(vec![
            Widget::draw_svg(ctx, "../data/system/assets/tools/layers.svg").margin_right(10),
            Line(format!("Population: {}", prettyprint_usize(total_ppl))).draw(ctx),
            Btn::plaintext("X")
                .build(ctx, "close", hotkey(Key::Escape))
                .align_right(),
        ]),
        Widget::row(vec![
            Widget::row(vec![
                Widget::draw_svg(ctx, "../data/system/assets/tools/home.svg").margin_right(10),
                Line(prettyprint_usize(ppl_in_bldg)).small().draw(ctx),
            ]),
            Line(format!("Off-map: {}", prettyprint_usize(ppl_off_map)))
                .small()
                .draw(ctx),
        ])
        .centered(),
    ];

    col.push(Checkbox::text(
        ctx,
        "Show heatmap",
        None,
        opts.heatmap.is_some(),
    ));
    if let Some(ref o) = opts.heatmap {
        col.extend(o.to_controls(ctx, colors_and_labels.unwrap()));
    }

    Composite::new(Widget::col(col).padding(5).bg(app.cs.panel_bg))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx)
}
