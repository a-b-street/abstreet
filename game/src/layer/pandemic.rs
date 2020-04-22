use crate::app::App;
use crate::common::{make_heatmap, HeatmapOptions};
use crate::layer::Layers;
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Btn, Checkbox, Choice, Color, Composite, EventCtx, GeomBatch, HorizontalAlignment, Key,
    TextExt, VerticalAlignment, Widget,
};
use geom::{Circle, Distance, Pt2D};
use sim::{GetDrawAgents, PersonState};
use std::collections::HashSet;

// TODO Disable drawing unzoomed agents... or alternatively, implement this by asking Sim to
// return this kind of data instead!
pub fn new(ctx: &mut EventCtx, app: &App, opts: Options) -> Layers {
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
    Layers::Pandemic(app.primary.sim.time(), opts, ctx.upload(batch), controls)
}

// TODO This should live in sim
#[derive(Clone, Copy, PartialEq)]
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

fn make_controls(
    ctx: &mut EventCtx,
    app: &App,
    opts: &Options,
    colors_and_labels: Option<(Vec<Color>, Vec<String>)>,
) -> Composite {
    let model = app.primary.sim.get_pandemic_model().unwrap();
    let pct = 100.0 / (model.count_total() as f64);

    let mut col = vec![
        Widget::row(vec![
            Widget::draw_svg(ctx, "../data/system/assets/tools/layers.svg").margin_right(10),
            "Pandemic model".draw_text(ctx),
            Btn::plaintext("X")
                .build(ctx, "close", hotkey(Key::Escape))
                .align_right(),
        ]),
        format!(
            "{} Sane ({:.1}%)",
            prettyprint_usize(model.count_sane()),
            (model.count_sane() as f64) * pct
        )
        .draw_text(ctx),
        format!(
            "{} Exposed ({:.1}%)",
            prettyprint_usize(model.count_exposed()),
            (model.count_exposed() as f64) * pct
        )
        .draw_text(ctx),
        format!(
            "{} Infected ({:.1}%)",
            prettyprint_usize(model.count_infected()),
            (model.count_infected() as f64) * pct
        )
        .draw_text(ctx),
        format!(
            "{} Recovered ({:.1}%)",
            prettyprint_usize(model.count_recovered()),
            (model.count_recovered() as f64) * pct
        )
        .draw_text(ctx),
        format!(
            "{} Dead ({:.1}%)",
            prettyprint_usize(model.count_dead()),
            (model.count_dead() as f64) * pct
        )
        .draw_text(ctx),
    ];
    col.push(Widget::row(vec![
        "Filter:".draw_text(ctx).margin_right(5),
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
    ]));

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

pub fn options(c: &Composite) -> Options {
    let heatmap = if c.is_checked("Show heatmap") {
        Some(HeatmapOptions::from_controls(c))
    } else {
        None
    };
    Options {
        heatmap,
        state: c.dropdown_value("seir"),
    }
}
