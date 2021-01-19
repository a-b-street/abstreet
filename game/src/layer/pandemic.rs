use std::collections::HashSet;

use abstutil::prettyprint_usize;
use geom::{Circle, Distance, Pt2D, Time};
use map_gui::tools::{make_heatmap, HeatmapOptions};
use sim::PersonState;
use widgetry::{
    Checkbox, Choice, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line,
    Outcome, Panel, StyledButtons, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::App;
use crate::layer::{Layer, LayerOutcome};

// TODO Disable drawing unzoomed agents... or alternatively, implement this by asking Sim to
// return this kind of data instead!
pub struct Pandemic {
    time: Time,
    opts: Options,
    draw: Drawable,
    panel: Panel,
}

impl Layer for Pandemic {
    fn name(&self) -> Option<&'static str> {
        Some("pandemic model")
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        minimap: &Panel,
    ) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            let mut new = Pandemic::new(ctx, app, self.opts.clone());
            new.panel.restore(ctx, &self.panel);
            *self = new;
        }

        self.panel.align_above(ctx, minimap);
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
                    *self = Pandemic::new(ctx, app, new_opts);
                    self.panel.align_above(ctx, minimap);
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

impl Pandemic {
    pub fn new(ctx: &mut EventCtx, app: &App, opts: Options) -> Pandemic {
        let model = app.primary.sim.get_pandemic_model().unwrap();

        let filter = |p| match opts.state {
            SEIR::Sane => model.is_sane(p),
            SEIR::Exposed => model.is_exposed(p),
            SEIR::Infected => model.is_exposed(p),
            SEIR::Recovered => model.is_recovered(p),
            SEIR::Dead => model.is_dead(p),
        };

        let mut pts = Vec::new();
        // Faster to grab all agent positions than individually map trips to agent positions.
        for a in app.primary.sim.get_unzoomed_agents(&app.primary.map) {
            if let Some(p) = a.person {
                if filter(p) {
                    pts.push(a.pos);
                }
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
                    if !filter(person.id) {
                        continue;
                    }

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
        Pandemic {
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
        Options {
            heatmap,
            state: self.panel.dropdown_value("seir"),
        }
    }
}

// TODO This should live in sim
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SEIR {
    Sane,
    Exposed,
    Infected,
    Recovered,
    Dead,
}

#[derive(Clone, PartialEq)]
pub struct Options {
    // If None, just a dot map
    pub heatmap: Option<HeatmapOptions>,
    pub state: SEIR,
}

fn make_controls(ctx: &mut EventCtx, app: &App, opts: &Options, legend: Option<Widget>) -> Panel {
    let model = app.primary.sim.get_pandemic_model().unwrap();
    let pct = 100.0 / (model.count_total() as f64);

    let mut col = vec![
        Widget::row(vec![
            Widget::draw_svg(ctx, "system/assets/tools/layers.svg"),
            "Pandemic model".draw_text(ctx),
            ctx.style().btn_close_widget(ctx),
        ]),
        Text::from_multiline(vec![
            Line(format!(
                "{} Sane ({:.1}%)",
                prettyprint_usize(model.count_sane()),
                (model.count_sane() as f64) * pct
            )),
            Line(format!(
                "{} Exposed ({:.1}%)",
                prettyprint_usize(model.count_exposed()),
                (model.count_exposed() as f64) * pct
            )),
            Line(format!(
                "{} Infected ({:.1}%)",
                prettyprint_usize(model.count_infected()),
                (model.count_infected() as f64) * pct
            )),
            Line(format!(
                "{} Recovered ({:.1}%)",
                prettyprint_usize(model.count_recovered()),
                (model.count_recovered() as f64) * pct
            )),
            Line(format!(
                "{} Dead ({:.1}%)",
                prettyprint_usize(model.count_dead()),
                (model.count_dead() as f64) * pct
            )),
        ])
        .draw(ctx),
        Widget::row(vec![
            "Filter:".draw_text(ctx),
            Widget::dropdown(
                ctx,
                "seir",
                opts.state,
                vec![
                    Choice::new("sane", SEIR::Sane),
                    Choice::new("exposed", SEIR::Exposed),
                    Choice::new("infected", SEIR::Infected),
                    Choice::new("recovered", SEIR::Recovered),
                    Choice::new("dead", SEIR::Dead),
                ],
            ),
        ]),
    ];

    col.push(Checkbox::switch(
        ctx,
        "Show heatmap",
        None,
        opts.heatmap.is_some(),
    ));
    if let Some(ref o) = opts.heatmap {
        col.extend(o.to_controls(ctx, legend.unwrap()));
    }

    Panel::new(Widget::col(col))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx)
}
