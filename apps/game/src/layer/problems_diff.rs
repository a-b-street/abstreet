use std::collections::HashMap;

use geom::{HashablePt2D, Polygon, Time};
use sim::Problem;
use widgetry::mapspace::ToggleZoomed;
use widgetry::{EventCtx, GfxCtx, Outcome, Panel, Toggle, Widget};

use crate::app::App;
use crate::layer::{header, problems, Layer, LayerOutcome, PANEL_PLACEMENT};

pub struct RelativeProblemMap {
    time: Time,
    opts: Options,
    draw: ToggleZoomed,
    panel: Panel,
}

impl Layer for RelativeProblemMap {
    fn name(&self) -> Option<&'static str> {
        Some("problem map")
    }
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            let mut new = Self::new(ctx, app, self.opts.clone());
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
            Outcome::Changed(x) => {
                if x == "Compare before proposal" {
                    return Some(LayerOutcome::Replace(Box::new(problems::ProblemMap::new(
                        ctx,
                        app,
                        problems::Options::new(app),
                    ))));
                }

                let new_opts = self.options();
                if self.opts != new_opts {
                    *self = Self::new(ctx, app, new_opts);
                }
            }
            _ => {}
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        self.draw.draw(g);
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.draw.unzoomed);
    }
}

impl RelativeProblemMap {
    pub fn new(ctx: &mut EventCtx, app: &App, opts: Options) -> Self {
        let mut count_per_pt: HashMap<HashablePt2D, isize> = HashMap::new();
        // Add for every problem occurring in this world
        for (_, problems) in &app.primary.sim.get_analytics().problems_per_trip {
            for (_, problem) in problems {
                if opts.show(problem) {
                    let pt = problem.point(&app.primary.map);
                    *count_per_pt.entry(pt.to_hashable()).or_insert(0) += 1;
                }
            }
        }
        // Subtract for every problem occurring in the baseline.
        let unedited_map = app
            .primary
            .unedited_map
            .as_ref()
            .unwrap_or(&app.primary.map);
        let now = app.primary.sim.time();
        for (_, problems) in &app.prebaked().problems_per_trip {
            for (time, problem) in problems {
                // Per trip, problems are counted in order, so stop after now.
                if *time > now {
                    break;
                }
                if opts.show(problem) {
                    let pt = problem.point(unedited_map);
                    *count_per_pt.entry(pt.to_hashable()).or_insert(0) -= 1;
                }
            }
        }

        // Assume there aren't outliers, and get the max change in problem count
        let max_count = count_per_pt
            .values()
            .map(|x| x.abs() as usize)
            .max()
            .unwrap_or(1) as f64;

        // Just draw colored rectangles (circles look too much like the unzoomed agents, so...
        let square = Polygon::rectangle(10.0, 10.0);
        let mut draw = ToggleZoomed::builder();

        for (pt, count) in count_per_pt {
            let pct = (count.abs() as f64) / max_count;
            let color = if count > 0 {
                app.cs.good_to_bad_red.eval(pct)
            } else if count < 0 {
                app.cs.good_to_bad_green.eval(pct)
            } else {
                continue;
            };
            let pt = pt.to_pt2d();
            let poly = square.translate(pt.x(), pt.y());
            draw.unzoomed.push(color, poly.clone());
            draw.zoomed.push(color.alpha(0.5), poly);
        }

        let controls = make_controls(ctx, &opts);
        Self {
            time: app.primary.sim.time(),
            opts,
            draw: draw.build(ctx),
            panel: controls,
        }
    }

    fn options(&self) -> Options {
        Options {
            show_delays: self.panel.is_checked("show delays"),
            show_complex_crossings: self
                .panel
                .is_checked("show where cyclists cross complex intersections"),
            show_overtakes: self
                .panel
                .is_checked("show where cars want to overtake cyclists"),
            show_arterial_crossings: self
                .panel
                .is_checked("show where pedestrians cross arterial intersections"),
            show_overcrowding: self
                .panel
                .is_checked("show where pedestrians are over-crowded"),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct Options {
    show_delays: bool,
    show_complex_crossings: bool,
    show_overtakes: bool,
    show_arterial_crossings: bool,
    show_overcrowding: bool,
}

impl Options {
    pub fn new() -> Options {
        Options {
            show_delays: true,
            show_complex_crossings: true,
            show_overtakes: true,
            show_arterial_crossings: true,
            show_overcrowding: true,
        }
    }

    fn show(&self, problem: &Problem) -> bool {
        match problem {
            Problem::IntersectionDelay(_, _) => self.show_delays,
            Problem::ComplexIntersectionCrossing(_) => self.show_complex_crossings,
            Problem::OvertakeDesired(_) => self.show_overtakes,
            Problem::ArterialIntersectionCrossing(_) => self.show_arterial_crossings,
            Problem::PedestrianOvercrowding(_) => self.show_overcrowding,
        }
    }
}

fn make_controls(ctx: &mut EventCtx, opts: &Options) -> Panel {
    let mut col = vec![
        header(ctx, "Change in Problems encountered"),
        Toggle::switch(ctx, "Compare before proposal", None, true),
    ];

    col.push(Toggle::checkbox(ctx, "show delays", None, opts.show_delays));
    col.push(Toggle::checkbox(
        ctx,
        "show where cyclists cross complex intersections",
        None,
        opts.show_complex_crossings,
    ));
    col.push(Toggle::checkbox(
        ctx,
        "show where cars want to overtake cyclists",
        None,
        opts.show_overtakes,
    ));
    col.push(Toggle::checkbox(
        ctx,
        "show where pedestrians cross arterial intersections",
        None,
        opts.show_arterial_crossings,
    ));
    col.push(Toggle::checkbox(
        ctx,
        "show where pedestrians are over-crowded",
        None,
        opts.show_overcrowding,
    ));

    Panel::new_builder(Widget::col(col))
        .aligned_pair(PANEL_PLACEMENT)
        .build(ctx)
}
