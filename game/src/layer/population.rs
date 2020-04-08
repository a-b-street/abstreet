use crate::app::App;
use crate::common::{make_heatmap, HeatmapOptions};
use crate::layer::Layers;
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Btn, Checkbox, Color, Composite, EventCtx, GeomBatch, HorizontalAlignment, Key, Line,
    Spinner, TextExt, VerticalAlignment, Widget,
};
use geom::{Circle, Distance, Pt2D};
use sim::{GetDrawAgents, PersonState, TripResult};
use std::collections::HashSet;

// TODO Disable drawing unzoomed agents... or alternatively, implement this by asking Sim to
// return this kind of data instead!
pub fn new(ctx: &mut EventCtx, app: &App, opts: Options) -> Layers {
    let filter = |p| {
        if let Some(pct) = opts.with_finished_trip_blocked_pct {
            // TODO This is probably inefficient...
            app.primary.sim.get_person(p).trips.iter().any(|t| {
                if let TripResult::TripDone = app.primary.sim.trip_to_agent(*t) {
                    let (total, blocked) = app.primary.sim.finished_trip_time(*t);
                    (100.0 * blocked / total) as usize >= pct
                } else {
                    false
                }
            })
        } else {
            true
        }
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
                if filter(person.id) {
                    let pt = app.primary.map.get_b(b).polygon.center();
                    if seen_bldgs.contains(&b) {
                        repeat_pts.push(pt);
                    } else {
                        seen_bldgs.insert(b);
                        pts.push(pt);
                    }
                }
            }
            PersonState::OffMap | PersonState::Limbo => {}
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
    Layers::PopulationMap(app.primary.sim.time(), opts, ctx.upload(batch), controls)
}

#[derive(Clone, PartialEq)]
pub struct Options {
    // If None, just a dot map
    pub heatmap: Option<HeatmapOptions>,
    // TODO More filters... Find people with finished/future trips of any/some mode
    pub with_finished_trip_blocked_pct: Option<usize>,
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
        "Filter by people with a finished trip",
        None,
        opts.with_finished_trip_blocked_pct.is_some(),
    ));
    if let Some(pct) = opts.with_finished_trip_blocked_pct {
        col.push(Widget::row(vec![
            "% of time spent waiting".draw_text(ctx).margin(5),
            Spinner::new(ctx, (0, 100), pct)
                .named("blocked ratio")
                .align_right()
                .centered_vert(),
        ]));
    }

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

pub fn options(c: &mut Composite) -> Options {
    let heatmap = if c.is_checked("Show heatmap") {
        Some(HeatmapOptions::from_controls(c))
    } else {
        None
    };
    Options {
        heatmap,
        with_finished_trip_blocked_pct: if c.is_checked("Filter by people with a finished trip") {
            if c.has_widget("blocked ratio") {
                Some(c.spinner("blocked ratio"))
            } else {
                // Just changed, use default
                Some(0)
            }
        } else {
            None
        },
    }
}
