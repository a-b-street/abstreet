use maplit::btreeset;

use geom::Duration;
use map_gui::tools::{grey_out_map, nice_map_name, open_browser, PopupMsg};
use sim::{AgentType, PersonID, TripID};
use widgetry::{
    lctrl, ControlState, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    SimpleState, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::jump_to_time_upon_startup;
use crate::edit::EditMode;
use crate::info::{OpenTrip, Tab};
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::{Actions, SandboxControls, SandboxMode};

/// A gameplay mode with specific controls for integration with
/// https://cyipt.github.io/acton/articles/the-actdev-project.html.
pub struct Actdev {
    top_center: Panel,
    scenario_name: String,
}

impl Actdev {
    pub fn new(ctx: &mut EventCtx, scenario_name: String) -> Box<dyn GameplayState> {
        Box::new(Actdev {
            top_center: Panel::empty(ctx),
            scenario_name,
        })
    }
}

impl GameplayState for Actdev {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        controls: &mut SandboxControls,
        actions: &mut Actions,
    ) -> Option<Transition> {
        match self.top_center.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "change scenario" => {
                    let scenario = if self.scenario_name == "base" {
                        "go_active"
                    } else {
                        "base"
                    };
                    return Some(Transition::Replace(SandboxMode::async_new(
                        app,
                        GameplayMode::Actdev(
                            app.primary.map.get_name().clone(),
                            scenario.to_string(),
                        ),
                        jump_to_time_upon_startup(Duration::hours(8)),
                    )));
                }
                "Edit map" => Some(Transition::Push(EditMode::new(
                    ctx,
                    app,
                    GameplayMode::Actdev(
                        app.primary.map.get_name().clone(),
                        self.scenario_name.clone(),
                    ),
                ))),
                "about A/B Street" => {
                    let panel = Panel::new(Widget::col(vec![
                        Widget::row(vec![
                            Line("About A/B Street").small_heading().draw(ctx),
                            ctx.style().btn_close_widget(ctx),
                        ]),
                        Line("Created by Dustin Carlino, Yuwen Li, & Michael Kirk")
                            .small()
                            .draw(ctx),
                        Text::from(Line(
                            "A/B Street is a traffic simulation game based on OpenStreetMap. You \
                             can modify roads and intersections, measure the effects on different \
                             groups, and advocate for your proposal.",
                        ))
                        .wrap_to_pct(ctx, 50)
                        .draw(ctx),
                        "This is a simplified version. Check out the full version below."
                            .draw_text(ctx),
                        ctx.style().btn_outline.text("abstreet.org").build_def(ctx),
                    ]))
                    .build(ctx);
                    Some(Transition::Push(SimpleState::new(panel, Box::new(About))))
                }
                "Follow someone" => {
                    if let Some((person, trip)) = find_active_trip(app) {
                        // The user may not realize they have to close layers; do it for them.
                        app.primary.layer = None;
                        ctx.canvas.cam_zoom = 40.0;
                        controls.common.as_mut().unwrap().launch_info_panel(
                            ctx,
                            app,
                            Tab::PersonTrips(person, OpenTrip::single(trip)),
                            actions,
                        );
                        None
                    } else {
                        return Some(Transition::Push(PopupMsg::new(
                            ctx,
                            "Nobody's around...",
                            vec!["There are no active trips right now"],
                        )));
                    }
                }
                "Cycling activity" => {
                    app.primary.layer =
                        Some(Box::new(crate::layer::map::BikeActivity::new(ctx, app)));
                    None
                }
                "Walking activity" => {
                    app.primary.layer = Some(Box::new(crate::layer::traffic::Throughput::new(
                        ctx,
                        app,
                        btreeset! { AgentType::Pedestrian },
                    )));
                    None
                }
                _ => unreachable!(),
            },
            _ => None,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_center.draw(g);
    }

    fn recreate_panels(&mut self, ctx: &mut EventCtx, app: &App) {
        let col = Widget::col(vec![
            Widget::row(vec![
                ctx.style()
                    .btn_plain
                    .btn()
                    .image_path("system/assets/pregame/logo.svg")
                    .image_dims(50.0)
                    .build_widget(ctx, "about A/B Street")
                    .centered_vert(),
                Line(nice_map_name(app.primary.map.get_name()))
                    .small_heading()
                    .draw(ctx),
                Widget::vert_separator(ctx, 50.0),
                ctx.style()
                    .btn_popup_icon_text("system/assets/tools/calendar.svg", "scenario")
                    .label_styled_text(
                        match self.scenario_name.as_ref() {
                            "base" => Text::from_all(vec![
                                Line("Baseline / "),
                                Line("Go Active").secondary(),
                            ]),
                            "go_active" => Text::from_all(vec![
                                Line("Baseline").secondary(),
                                Line(" / Go Active"),
                            ]),
                            _ => unreachable!(),
                        },
                        ControlState::Default,
                    )
                    .build_widget(ctx, "change scenario"),
                ctx.style()
                    .btn_outline
                    .icon_text("system/assets/tools/pencil.svg", "Edit map")
                    .hotkey(lctrl(Key::E))
                    .build_def(ctx),
            ])
            .centered(),
            Widget::row(vec![
                ctx.style()
                    .btn_plain
                    .icon_text("system/assets/tools/location.svg", "Follow someone")
                    .build_def(ctx),
                ctx.style()
                    .btn_plain
                    .icon_text("system/assets/meters/bike.svg", "Cycling activity")
                    .build_def(ctx),
                ctx.style()
                    .btn_plain
                    .icon_text("system/assets/meters/pedestrian.svg", "Walking activity")
                    .build_def(ctx),
            ]),
        ]);

        self.top_center = Panel::new(col)
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx);
    }

    fn has_tool_panel(&self) -> bool {
        // Get rid of the home button, which would allow escaping to the title screen
        false
    }
}

struct About;

impl SimpleState<App> for About {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, x: &str, _: &Panel) -> Transition {
        if x == "close" {
            return Transition::Pop;
        } else if x == "abstreet.org" {
            open_browser("https://abstreet.org");
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
    }
}

fn find_active_trip(app: &App) -> Option<(PersonID, TripID)> {
    for agent in app.primary.sim.active_agents() {
        if let Some(trip) = app.primary.sim.agent_to_trip(agent) {
            if let Some(person) = app.primary.sim.trip_to_person(trip) {
                return Some((person, trip));
            }
        }
    }
    None
}
