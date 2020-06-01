use crate::app::App;
use crate::common::{CityPicker, CommonState};
use crate::edit::EditMode;
use crate::game::{State, Transition, WizardState};
use crate::helpers::nice_map_name;
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::SandboxControls;
use crate::sandbox::SandboxMode;
use ezgui::{
    hotkey, lctrl, Btn, Choice, Color, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment,
    Key, Line, Outcome, ScreenRectangle, TextExt, VerticalAlignment, Widget,
};
use geom::Polygon;
use map_model::IntersectionID;
use sim::{TripEndpoint, TripMode};
use std::collections::BTreeSet;

// TODO Maybe remember what things were spawned, offer to replay this later
pub struct Freeform {
    // TODO Clean these up later when done?
    pub spawn_pts: BTreeSet<IntersectionID>,
    top_center: Composite,
}

impl Freeform {
    pub fn new(ctx: &mut EventCtx, app: &App, mode: GameplayMode) -> Box<dyn GameplayState> {
        Box::new(Freeform {
            spawn_pts: BTreeSet::new(),
            top_center: freeform_controller(ctx, app, mode, "none"),
        })
    }
}

impl GameplayState for Freeform {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        _: &mut SandboxControls,
    ) -> Option<Transition> {
        match self.top_center.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "change map" => {
                    Some(Transition::Push(CityPicker::new(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            // The map will be switched before this callback happens.
                            let path = abstutil::path_map(app.primary.map.get_name());
                            Transition::PopThenReplace(Box::new(SandboxMode::new(
                                ctx,
                                app,
                                GameplayMode::Freeform(path),
                            )))
                        }),
                    )))
                }
                "change traffic" => Some(Transition::Push(make_change_traffic(
                    self.top_center.rect_of("change traffic").clone(),
                ))),
                "edit map" => Some(Transition::Push(Box::new(EditMode::new(
                    ctx,
                    app,
                    GameplayMode::Freeform(abstutil::path_map(app.primary.map.get_name())),
                )))),
                "Start a new trip" => Some(Transition::Push(AgentSpawner::new(ctx, app))),
                _ => unreachable!(),
            },
            None => None,
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.top_center.draw(g);
        // TODO Overriding draw options would be ideal, but...
        for i in &self.spawn_pts {
            g.draw_polygon(Color::GREEN.alpha(0.8), &app.primary.map.get_i(*i).polygon);
        }
    }
}

pub fn freeform_controller(
    ctx: &mut EventCtx,
    app: &App,
    gameplay: GameplayMode,
    scenario_name: &str,
) -> Composite {
    let mut rows = vec![Widget::row(vec![
        Line("Sandbox").small_heading().draw(ctx).margin(5),
        Widget::draw_batch(
            ctx,
            GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 50.0))]),
        )
        .margin(5),
        "Map:".draw_text(ctx).margin(5),
        Btn::text_fg(format!("{} ▼", nice_map_name(app.primary.map.get_name())))
            .build(ctx, "change map", lctrl(Key::L))
            .margin(5),
        "Traffic:".draw_text(ctx).margin(5),
        Btn::text_fg(format!("{} ▼", scenario_name))
            .build(ctx, "change traffic", hotkey(Key::S))
            .margin(5),
        Btn::svg_def("../data/system/assets/tools/edit_map.svg")
            .build(ctx, "edit map", lctrl(Key::E))
            .margin(5),
    ])
    .centered()];
    if let GameplayMode::Freeform(_) = gameplay {
        rows.push(
            Btn::text_fg("Start a new trip")
                .build_def(ctx, None)
                .centered_horiz(),
        );
    }

    Composite::new(Widget::col(rows).bg(app.cs.panel_bg).padding(10))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx)
}

pub fn make_change_traffic(btn: ScreenRectangle) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, app| {
        let (_, scenario_name) = wiz.wrap(ctx).choose_exact(
            (
                HorizontalAlignment::Centered(btn.center().x),
                VerticalAlignment::Below(btn.y2 + 15.0),
            ),
            None,
            || {
                let mut list = Vec::new();
                for name in abstutil::list_all_objects(abstutil::path_all_scenarios(
                    app.primary.map.get_name(),
                )) {
                    if name == "weekday" {
                        list.push(Choice::new("realistic weekday traffic", name).tooltip(
                            "Trips will begin throughout the entire day. Midnight is usually \
                             quiet, so you may need to fast-forward to morning rush hour. Data \
                             comes from Puget Sound Regional Council's Soundcast model.",
                        ));
                        list.push(
                            Choice::new("5 weekdays repeated", "5 weekdays repeated".to_string())
                                .tooltip(
                                    "Same as the weekday traffic pattern, but blindly repeated 5 \
                                     times. This isn't realistic; people don't take exactly the \
                                     same trips every day.",
                                ),
                        );
                    } else {
                        list.push(Choice::new(name.clone(), name));
                    }
                }
                list.push(
                    Choice::new("random unrealistic trips", "random".to_string()).tooltip(
                        "Lots of trips will start at midnight, but not constantly appear through \
                         the day.",
                    ),
                );
                list.push(Choice::new("just buses", "just buses".to_string()));
                list.push(Choice::new(
                    "none (you manually spawn traffic)",
                    "empty".to_string(),
                ));
                list
            },
        )?;
        let map_path = abstutil::path_map(app.primary.map.get_name());
        Some(Transition::PopThenReplace(Box::new(SandboxMode::new(
            ctx,
            app,
            if scenario_name == "empty" {
                GameplayMode::Freeform(map_path)
            } else {
                GameplayMode::PlayScenario(map_path, scenario_name)
            },
        ))))
    }))
}

struct AgentSpawner {
    composite: Composite,
    _source: Option<TripEndpoint>,
}

impl AgentSpawner {
    fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        Box::new(AgentSpawner {
            _source: None,
            composite: Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        Line("New trip").small_heading().draw(ctx),
                        Btn::plaintext("X")
                            .build(ctx, "close", hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    "Click a building or border to specify start"
                        .draw_text(ctx)
                        .named("instructions"),
                    Widget::row(vec![
                        "Type of trip:".draw_text(ctx).margin_right(10),
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
                ])
                .bg(app.cs.panel_bg)
                .padding(10),
            )
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
        })
    }
}

impl State for AgentSpawner {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.composite.draw(g);
        CommonState::draw_osd(g, app, &app.primary.current_selection);
    }
}
