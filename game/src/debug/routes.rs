use map_gui::ID;
use map_model::NORMAL_LANE_THICKNESS;
use sim::{TripEndpoint, TripMode};
use widgetry::{
    Choice, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, Outcome,
    Panel, State, StyledButtons, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::CommonState;

pub struct RouteExplorer {
    panel: Panel,
    start: TripEndpoint,
    // (endpoint, confirmed, render the paths to it)
    goal: Option<(TripEndpoint, bool, Drawable)>,
}

impl RouteExplorer {
    pub fn new(ctx: &mut EventCtx, start: TripEndpoint) -> Box<dyn State<App>> {
        Box::new(RouteExplorer {
            start,
            goal: None,
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Route explorer").small_heading().draw(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Widget::row(vec![
                    "Type of trip:".draw_text(ctx),
                    Widget::dropdown(
                        ctx,
                        "mode",
                        TripMode::Drive,
                        TripMode::all()
                            .into_iter()
                            .map(|m| Choice::new(m.ongoing_verb(), m))
                            .collect(),
                    ),
                ]),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
        })
    }

    fn recalc_paths(&mut self, ctx: &mut EventCtx, app: &App) {
        if let Some((ref goal, _, ref mut preview)) = self.goal {
            *preview = Drawable::empty(ctx);
            if let Some(polygon) = TripEndpoint::path_req(
                self.start.clone(),
                goal.clone(),
                self.panel.dropdown_value("mode"),
                &app.primary.map,
            )
            .and_then(|req| app.primary.map.pathfind(req).ok())
            .and_then(|path| path.trace(&app.primary.map))
            .map(|pl| pl.make_polygons(NORMAL_LANE_THICKNESS))
            {
                *preview = GeomBatch::from(vec![(Color::PURPLE, polygon)]).upload(ctx);
            }
        }
    }
}

impl State<App> for RouteExplorer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            Outcome::Changed => {
                // Mode choice changed
                self.recalc_paths(ctx, app);
            }
            _ => {}
        }

        if self
            .goal
            .as_ref()
            .map(|(_, confirmed, _)| *confirmed)
            .unwrap_or(false)
        {
            return Transition::Keep;
        }

        if ctx.redo_mouseover() {
            app.primary.current_selection = app.mouseover_unzoomed_everything(ctx);
            if match app.primary.current_selection {
                Some(ID::Intersection(i)) => !app.primary.map.get_i(i).is_border(),
                Some(ID::Building(_)) => false,
                _ => true,
            } {
                app.primary.current_selection = None;
            }
        }
        if let Some(hovering) = match app.primary.current_selection {
            Some(ID::Intersection(i)) => Some(TripEndpoint::Border(i)),
            Some(ID::Building(b)) => Some(TripEndpoint::Bldg(b)),
            None => None,
            _ => unreachable!(),
        } {
            if self.start != hovering {
                if self
                    .goal
                    .as_ref()
                    .map(|(to, _, _)| to != &hovering)
                    .unwrap_or(true)
                {
                    self.goal = Some((hovering, false, Drawable::empty(ctx)));
                    self.recalc_paths(ctx, app);
                }
            } else {
                self.goal = None;
            }
        } else {
            self.goal = None;
        }

        if let Some((_, ref mut confirmed, _)) = self.goal {
            if app.per_obj.left_click(ctx, "end here") {
                app.primary.current_selection = None;
                *confirmed = true;
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        CommonState::draw_osd(g, app);

        g.draw_polygon(
            Color::BLUE.alpha(0.8),
            match self.start {
                TripEndpoint::Border(i) => app.primary.map.get_i(i).polygon.clone(),
                TripEndpoint::Bldg(b) => app.primary.map.get_b(b).polygon.clone(),
                TripEndpoint::SuddenlyAppear(_) => unreachable!(),
            },
        );
        if let Some((ref endpt, _, ref draw)) = self.goal {
            g.draw_polygon(
                Color::GREEN.alpha(0.8),
                match endpt {
                    TripEndpoint::Border(i) => app.primary.map.get_i(*i).polygon.clone(),
                    TripEndpoint::Bldg(b) => app.primary.map.get_b(*b).polygon.clone(),
                    TripEndpoint::SuddenlyAppear(_) => unreachable!(),
                },
            );
            g.redraw(draw);
        }
    }
}
