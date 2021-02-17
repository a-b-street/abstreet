use map_gui::tools::{grey_out_map, nice_map_name, open_browser, PopupMsg};
use sim::{PersonID, TripID};
use widgetry::{
    lctrl, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, SimpleState,
    StyledButtons, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::edit::EditMode;
use crate::info::{OpenTrip, Tab};
use crate::sandbox::gameplay::freeform::ChangeScenario;
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::{Actions, SandboxControls};

pub struct Blog {
    top_center: Panel,
    scenario_name: Option<String>,
}

impl Blog {
    pub fn new(ctx: &mut EventCtx, scenario_name: Option<String>) -> Box<dyn GameplayState> {
        Box::new(Blog {
            top_center: Panel::empty(ctx),
            scenario_name,
        })
    }
}

impl GameplayState for Blog {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        controls: &mut SandboxControls,
        actions: &mut Actions,
    ) -> Option<Transition> {
        match self.top_center.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                // TODO This'll bring us out of this GameplayMode.
                "change scenario" => Some(Transition::Push(ChangeScenario::new(ctx, app, "none"))),
                "edit map" => Some(Transition::Push(EditMode::new(
                    ctx,
                    app,
                    GameplayMode::Freeform(app.primary.map.get_name().clone()),
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
                        ctx.style()
                            .btn_outline_light_text("abstreet.org")
                            .build_def(ctx),
                    ]))
                    .build(ctx);
                    Some(Transition::Push(SimpleState::new(panel, Box::new(About))))
                }
                "follow someone" => {
                    if let Some((person, trip)) = find_active_trip(app) {
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
                "bike network" => {
                    app.primary.layer =
                        Some(Box::new(crate::layer::map::BikeNetwork::new(ctx, app)));
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
                    .btn_plain_light()
                    .image_path("system/assets/pregame/logo.svg")
                    .image_dims(50.0)
                    .build_widget(ctx, "about A/B Street")
                    .centered_vert(),
                Line(nice_map_name(app.primary.map.get_name()))
                    .small_heading()
                    .draw(ctx),
                Widget::vert_separator(ctx, 50.0),
                ctx.style()
                    .btn_light_popup_icon_text(
                        "system/assets/tools/calendar.svg",
                        self.scenario_name.as_ref().unwrap_or(&"none".to_string()),
                    )
                    .hotkey(Key::S)
                    .build_widget(ctx, "change scenario"),
                ctx.style()
                    .btn_outline_light_icon_text("system/assets/tools/pencil.svg", "Edit map")
                    .hotkey(lctrl(Key::E))
                    .build_widget(ctx, "edit map"),
            ])
            .centered(),
            Widget::row(vec![
                ctx.style()
                    .btn_plain_light_icon_text("system/assets/tools/location.svg", "Follow someone")
                    .build_widget(ctx, "follow someone"),
                ctx.style()
                    .btn_plain_light_icon_text("system/assets/meters/bike.svg", "Bike network")
                    .build_widget(ctx, "bike network"),
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
