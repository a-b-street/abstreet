use crate::app::App;
use crate::colors::HeatmapColors;
use crate::common::{make_heatmap, ColorLegend, HeatmapOptions};
use crate::layer::Layers;
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Btn, Checkbox, Color, Composite, EventCtx, GeomBatch, HorizontalAlignment, Key, Line,
    Spinner, TextExt, VerticalAlignment, Widget,
};
use geom::{Circle, Distance, Pt2D};
use sim::{GetDrawAgents, PersonState};
use std::collections::HashSet;

// TODO Disable drawing unzoomed agents... or alternatively, implement this by asking Sim to
// return this kind of data instead!
pub fn new(ctx: &mut EventCtx, app: &App, opts: PopulationOptions) -> Layers {
    // Only display infected people if this is enabled.
    let maybe_pandemic = if opts.pandemic {
        app.primary.sim.get_pandemic_model()
    } else {
        None
    };

    let mut pts = Vec::new();
    // Faster to grab all agent positions than individually map trips to agent positions.
    if let Some(ref model) = maybe_pandemic {
        for a in app.primary.sim.get_unzoomed_agents(&app.primary.map) {
            if let Some(p) = a.person {
                if model.infected.contains_key(&p) {
                    pts.push(a.pos);
                }
            }
        }
    } else {
        for a in app.primary.sim.get_unzoomed_agents(&app.primary.map) {
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
                if maybe_pandemic
                    .as_ref()
                    .map(|m| !m.infected.contains_key(&person.id))
                    .unwrap_or(false)
                {
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
    let controls = population_controls(ctx, app, &opts, colors_and_labels);
    Layers::PopulationMap(app.primary.sim.time(), opts, ctx.upload(batch), controls)
}

#[derive(Clone, PartialEq)]
pub struct PopulationOptions {
    pub pandemic: bool,
    // If None, just a dot map
    pub heatmap: Option<HeatmapOptions>,
}

// This function sounds more ominous than it should.
fn population_controls(
    ctx: &mut EventCtx,
    app: &App,
    opts: &PopulationOptions,
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
        if app.primary.sim.get_pandemic_model().is_some() {
            Checkbox::text(ctx, "Show pandemic model", None, opts.pandemic)
        } else {
            Widget::nothing()
        },
    ];

    if opts.pandemic {
        let model = app.primary.sim.get_pandemic_model().unwrap();
        col.push(
            format!(
                "Pandemic model: {} S ({:.1}%),  {} E ({:.1}%),  {} I ({:.1}%),  {} R ({:.1}%)",
                prettyprint_usize(model.count_sane()),
                (model.count_sane() as f64) / (total_ppl as f64) * 100.0,
                prettyprint_usize(model.count_exposed()),
                (model.count_exposed() as f64) / (total_ppl as f64) * 100.0,
                prettyprint_usize(model.count_infected()),
                (model.count_infected() as f64) / (total_ppl as f64) * 100.0,
                prettyprint_usize(model.count_recovered()),
                (model.count_recovered() as f64) / (total_ppl as f64) * 100.0
            )
            .draw_text(ctx),
        );
        assert_eq!(total_ppl, model.count_total());
    }

    col.push(Checkbox::text(
        ctx,
        "Show heatmap",
        None,
        opts.heatmap.is_some(),
    ));
    if let Some(ref o) = opts.heatmap {
        // TODO Display the value...
        col.push(Widget::row(vec![
            "Resolution (meters)".draw_text(ctx).margin(5),
            Spinner::new(ctx, (1, 100), o.resolution)
                .named("resolution")
                .align_right()
                .centered_vert(),
        ]));
        col.push(Widget::row(vec![
            "Radius (resolution multiplier)".draw_text(ctx).margin(5),
            Spinner::new(ctx, (0, 10), o.radius)
                .named("radius")
                .align_right()
                .centered_vert(),
        ]));

        col.push(Widget::row(vec![
            "Color scheme".draw_text(ctx).margin(5),
            Widget::dropdown(ctx, "Colors", o.colors, HeatmapColors::choices()),
        ]));

        // Legend for the heatmap colors
        let (colors, labels) = colors_and_labels.unwrap();
        col.push(ColorLegend::scale(ctx, colors, labels));
    }

    Composite::new(Widget::col(col).padding(5).bg(app.cs.panel_bg))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx)
}

pub fn population_options(c: &mut Composite) -> PopulationOptions {
    let heatmap = if c.is_checked("Show heatmap") {
        // Did we just change?
        if c.has_widget("resolution") {
            Some(HeatmapOptions {
                resolution: c.spinner("resolution"),
                radius: c.spinner("radius"),
                colors: c.dropdown_value("Colors"),
            })
        } else {
            Some(HeatmapOptions::new())
        }
    } else {
        None
    };
    PopulationOptions {
        pandemic: c.has_widget("Show pandemic model") && c.is_checked("Show pandemic model"),
        heatmap,
    }
}
