use std::collections::HashSet;

use abstutil::{prettyprint_usize, Counter};
use geom::Time;
use map_gui::tools::{ColorNetwork, DivergingScale};
use map_gui::ID;
use map_model::{IntersectionID, RoadID, Traversable};
use sim::{Problem, ProblemType};
use widgetry::mapspace::ToggleZoomed;
use widgetry::{Color, EventCtx, GfxCtx, Outcome, Panel, Text, Toggle, Widget};

use crate::app::App;
use crate::layer::{header, problems, Layer, LayerOutcome, PANEL_PLACEMENT};

pub struct RelativeProblemMap {
    time: Time,
    opts: Options,
    draw: ToggleZoomed,
    panel: Panel,

    before_road: Counter<RoadID>,
    before_intersection: Counter<IntersectionID>,
    after_road: Counter<RoadID>,
    after_intersection: Counter<IntersectionID>,
    tooltip: Option<Text>,
}

impl Layer for RelativeProblemMap {
    fn name(&self) -> Option<&'static str> {
        Some("problem map")
    }
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<LayerOutcome> {
        let mut recalc_tooltip = false;
        if app.primary.sim.time() != self.time {
            let mut new = Self::new(ctx, app, self.opts.clone());
            new.panel.restore(ctx, &self.panel);
            *self = new;
            recalc_tooltip = true;
        }

        // TODO Reinventing CompareCounts...
        if ctx.redo_mouseover() || recalc_tooltip {
            self.tooltip = None;
            match app.mouseover_unzoomed_roads_and_intersections(ctx) {
                Some(ID::Road(r)) => {
                    let (before, after) = (self.before_road.get(r), self.after_road.get(r));
                    self.tooltip = Some(Text::from(format!(
                        "{} before, {} after",
                        prettyprint_usize(before),
                        prettyprint_usize(after)
                    )));
                }
                Some(ID::Intersection(i)) => {
                    let (before, after) = (
                        self.before_intersection.get(i),
                        self.after_intersection.get(i),
                    );
                    self.tooltip = Some(Text::from(format!(
                        "{} before, {} after",
                        prettyprint_usize(before),
                        prettyprint_usize(after)
                    )));
                }
                _ => {}
            }
        } else {
            self.tooltip = None;
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
        if let Some(ref txt) = self.tooltip {
            g.draw_mouse_tooltip(txt.clone());
        }
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.draw.unzoomed);
    }
}

impl RelativeProblemMap {
    pub fn new(ctx: &mut EventCtx, app: &App, opts: Options) -> Self {
        let after = app.primary.sim.get_analytics();
        let before = app.prebaked();
        let now = app.primary.sim.time();

        let mut after_road = Counter::new();
        let mut before_road = Counter::new();
        let mut after_intersection = Counter::new();
        let mut before_intersection = Counter::new();

        let update_count =
            |problem: &Problem,
             roads: &mut Counter<RoadID>,
             intersections: &mut Counter<IntersectionID>| {
                match problem {
                    Problem::IntersectionDelay(i, _) | Problem::ComplexIntersectionCrossing(i) => {
                        intersections.inc(*i);
                    }
                    Problem::OvertakeDesired(on) | Problem::PedestrianOvercrowding(on) => {
                        match on {
                            Traversable::Lane(l) => {
                                roads.inc(l.road);
                            }
                            Traversable::Turn(t) => {
                                intersections.inc(t.parent);
                            }
                        }
                    }
                    Problem::ArterialIntersectionCrossing(t) => {
                        intersections.inc(t.parent);
                    }
                }
            };

        for (_, problems) in &before.problems_per_trip {
            for (time, problem) in problems {
                // Per trip, problems are counted in order, so stop after now.
                if *time > now {
                    break;
                }
                if opts.show(problem) {
                    update_count(problem, &mut before_road, &mut before_intersection);
                }
            }
        }

        for (_, problems) in &after.problems_per_trip {
            for (_, problem) in problems {
                if opts.show(problem) {
                    update_count(problem, &mut after_road, &mut after_intersection);
                }
            }
        }

        let mut colorer = ColorNetwork::new(app);

        let scale = DivergingScale::new(Color::hex("#5D9630"), Color::WHITE, Color::hex("#A32015"))
            .range(0.0, 2.0)
            .ignore(0.7, 1.3);

        for (r, before, after) in before_road.clone().compare(after_road.clone()) {
            if let Some(c) = scale.eval((after as f64) / (before as f64)) {
                colorer.add_r(r, c);
            }
        }
        for (i, before, after) in before_intersection
            .clone()
            .compare(after_intersection.clone())
        {
            if let Some(c) = scale.eval((after as f64) / (before as f64)) {
                colorer.add_i(i, c);
            }
        }

        let legend = scale.make_legend(ctx, vec!["less problems", "same", "more"]);
        let controls = make_controls(ctx, &opts, legend);
        Self {
            time: app.primary.sim.time(),
            opts,
            draw: colorer.build(ctx),
            panel: controls,
            tooltip: None,
            before_road,
            before_intersection,
            after_road,
            after_intersection,
        }
    }

    fn options(&self) -> Options {
        ProblemTypes::from_controls(&self.panel)
    }
}

fn make_controls(ctx: &mut EventCtx, opts: &Options, legend: Widget) -> Panel {
    Panel::new_builder(Widget::col(vec![
        header(ctx, "Change in Problems encountered"),
        Toggle::switch(ctx, "Compare before proposal", None, true),
        opts.to_controls(ctx),
        legend,
    ]))
    .aligned_pair(PANEL_PLACEMENT)
    .build(ctx)
}

pub type Options = ProblemTypes;

#[derive(Clone, PartialEq)]
pub struct ProblemTypes {
    disabled_types: HashSet<ProblemType>,
}

impl ProblemTypes {
    pub fn new() -> Self {
        Self {
            disabled_types: HashSet::new(),
        }
    }

    pub fn show(&self, problem: &Problem) -> bool {
        !self.disabled_types.contains(&ProblemType::from(problem))
    }

    pub fn to_controls(&self, ctx: &mut EventCtx) -> Widget {
        let mut col = Vec::new();
        for pt in ProblemType::all() {
            col.push(Toggle::checkbox(
                ctx,
                &format!("show {}", pt.name()),
                None,
                !self.disabled_types.contains(&pt),
            ));
        }
        Widget::col(col)
    }

    pub fn from_controls(panel: &Panel) -> Self {
        let mut types = Self::new();
        for pt in ProblemType::all() {
            if !panel.is_checked(&format!("show {}", pt.name())) {
                types.disabled_types.insert(pt);
            }
        }
        types
    }
}
