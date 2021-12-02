use abstutil::Counter;
use map_gui::tools::{ColorLegend, ColorNetwork};
use map_gui::ID;
use map_model::{IntersectionID, PathStep, RoadID, Traversable};
use widgetry::mapspace::ToggleZoomed;
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Line, Outcome, Panel, State, Text,
    VerticalAlignment, Widget,
};

use crate::app::App;
use crate::app::Transition;
use crate::common::CommonState;

/// A state to count the number of trips that will cross different roads.
pub struct PathCounter {
    panel: Panel,
    draw: ToggleZoomed,
    cnt: Counter<RoadID>,
    tooltip: Option<Text>,
}

impl PathCounter {
    pub fn demand_across_intersection(
        ctx: &mut EventCtx,
        app: &App,
        i: IntersectionID,
    ) -> Box<dyn State<App>> {
        let map = &app.primary.map;
        let sim = &app.primary.sim;
        let mut cnt = Counter::new();
        // Find all active agents whose path crosses this intersection
        for agent in sim.active_agents() {
            if let Some(path) = sim.get_path(agent) {
                if path.get_steps().iter().any(|step| match step {
                    PathStep::Turn(t) | PathStep::ContraflowTurn(t) => t.parent == i,
                    _ => false,
                }) {
                    // Count what lanes they'll cross
                    for step in path.get_steps() {
                        if let Traversable::Lane(l) = step.as_traversable() {
                            cnt.inc(l.road);
                        }
                    }
                }
            }
        }

        let mut colorer = ColorNetwork::new(app);
        // Highlight the intersection
        colorer
            .unzoomed
            .push(Color::CYAN, map.get_i(i).polygon.clone());
        colorer
            .zoomed
            .push(Color::CYAN.alpha(0.5), map.get_i(i).polygon.clone());

        colorer.pct_roads(cnt.clone(), &app.cs.good_to_bad_red);

        Box::new(PathCounter {
            draw: colorer.build(ctx),
            tooltip: None,
            cnt,
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line(format!("Paths across {}", i))
                        .small_heading()
                        .into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                ColorLegend::gradient(
                    ctx,
                    &app.cs.good_to_bad_red,
                    vec!["lowest count", "highest"],
                ),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
        })
    }
}

impl State<App> for PathCounter {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.primary.current_selection = app.mouseover_unzoomed_roads_and_intersections(ctx);
            self.tooltip = None;
            if let Some(r) = match app.primary.current_selection {
                Some(ID::Lane(l)) => Some(l.road),
                Some(ID::Road(r)) => Some(r),
                _ => None,
            } {
                let n = self.cnt.get(r);
                if n > 0 {
                    self.tooltip = Some(Text::from(abstutil::prettyprint_usize(n)));
                }
            } else {
                app.primary.current_selection = None;
            }
        }

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        CommonState::draw_osd(g, app);

        self.draw.draw(g);

        if let Some(ref txt) = self.tooltip {
            g.draw_mouse_tooltip(txt.clone());
        }
    }
}
