use crate::app::App;
use crate::colors;
use crate::edit::EditMode;
use crate::game::{State, Transition, WizardState};
use crate::helpers::{nice_map_name, ID};
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::sandbox::gameplay::{spawner, GameplayMode, GameplayState};
use crate::sandbox::SandboxControls;
use crate::sandbox::SandboxMode;
use ezgui::{
    hotkey, lctrl, Btn, Choice, Color, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment,
    Key, Line, ScreenRectangle, Text, VerticalAlignment, Widget,
};
use geom::Polygon;
use map_model::IntersectionID;
use std::collections::BTreeSet;

// TODO Maybe remember what things were spawned, offer to replay this later
pub struct Freeform {
    // TODO Clean these up later when done?
    pub spawn_pts: BTreeSet<IntersectionID>,
    top_center: WrappedComposite,
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
    ) -> (Option<Transition>, bool) {
        match self.top_center.event(ctx, app) {
            Some(WrappedOutcome::Transition(t)) => {
                return (Some(t), false);
            }
            Some(WrappedOutcome::Clicked(_)) => unreachable!(),
            None => {}
        }

        if let Some(new_state) = spawner::AgentSpawner::new(ctx, app) {
            return (Some(Transition::Push(new_state)), false);
        }
        if let Some(new_state) = spawner::SpawnManyAgents::new(ctx, app) {
            return (Some(Transition::Push(new_state)), false);
        }
        (None, false)
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.top_center.draw(g);
        // TODO Overriding draw options would be ideal, but...
        for i in &self.spawn_pts {
            g.draw_polygon(Color::GREEN.alpha(0.8), &app.primary.map.get_i(*i).polygon);
        }

        if let Some(ID::Intersection(i)) = app.primary.current_selection {
            if self.spawn_pts.contains(&i) {
                let mut txt = Text::new();
                for line in app.primary.sim.count_trips_involving_border(i).describe() {
                    txt.add(Line(line));
                }
                if !txt.is_empty() {
                    g.draw_mouse_tooltip(txt);
                }
            }
        }
    }
}

pub fn freeform_controller(
    ctx: &mut EventCtx,
    app: &App,
    gameplay: GameplayMode,
    scenario_name: &str,
) -> WrappedComposite {
    let c = Composite::new(
        Widget::row(vec![
            Line("Sandbox").size(26).draw(ctx).margin(5),
            Widget::draw_batch(
                ctx,
                GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 50.0))]),
            )
            .margin(5),
            Text::from(Line("Map:").size(18).roboto_bold())
                .draw(ctx)
                .margin(5),
            Btn::custom_text_fg(Text::from(
                Line(format!("{} ▼", nice_map_name(app.primary.map.get_name())))
                    .size(18)
                    .roboto(),
            ))
            .build(ctx, "change map", lctrl(Key::L))
            .margin(5),
            Text::from(Line("Traffic:").size(18).roboto_bold())
                .draw(ctx)
                .margin(5),
            Btn::custom_text_fg(Text::from(
                Line(format!("{} ▼", scenario_name)).size(18).roboto(),
            ))
            .build(ctx, "change traffic", hotkey(Key::S))
            .margin(5),
            Btn::svg_def("../data/system/assets/tools/edit_map.svg")
                .build(ctx, "edit map", lctrl(Key::E))
                .margin(5),
        ])
        .centered()
        .bg(colors::PANEL_BG),
    )
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx);
    let map_picker = c.rect_of("change map").clone();
    let traffic_picker = c.rect_of("change traffic").clone();

    WrappedComposite::new(c)
        .cb("change map", {
            let gameplay = gameplay.clone();
            Box::new(move |_, _| {
                Some(Transition::Push(make_load_map(
                    map_picker.clone(),
                    gameplay.clone(),
                )))
            })
        })
        .cb(
            "change traffic",
            Box::new(move |_, _| {
                Some(Transition::Push(make_change_traffic(
                    traffic_picker.clone(),
                )))
            }),
        )
        .cb(
            "edit map",
            Box::new(move |ctx, app| {
                Some(Transition::Push(Box::new(EditMode::new(
                    ctx,
                    app,
                    gameplay.clone(),
                ))))
            }),
        )
}

fn make_load_map(btn: ScreenRectangle, gameplay: GameplayMode) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, app| {
        if let Some((_, name)) = wiz.wrap(ctx).choose_exact(
            (
                HorizontalAlignment::Centered(btn.center().x),
                VerticalAlignment::Below(btn.y2 + 15.0),
            ),
            None,
            || {
                let current_map = app.primary.map.get_name();
                abstutil::list_all_objects(abstutil::path_all_maps())
                    .into_iter()
                    .filter(|n| n != current_map)
                    .map(|n| {
                        let c = Choice::new(nice_map_name(&n), n.clone());
                        // Hardcoded list for now.
                        if n == "montlake" || n == "23rd" {
                            c
                        } else {
                            c.tooltip(
                                "This map currently has bugs causing unrealistic traffic jams.",
                            )
                        }
                    })
                    .collect()
            },
        ) {
            Some(Transition::PopThenReplace(Box::new(SandboxMode::new(
                ctx,
                app,
                match gameplay {
                    GameplayMode::Freeform(_) => GameplayMode::Freeform(abstutil::path_map(&name)),
                    // Assume a scenario with the same name exists.
                    GameplayMode::PlayScenario(_, ref scenario) => {
                        GameplayMode::PlayScenario(abstutil::path_map(&name), scenario.clone())
                    }
                    _ => unreachable!(),
                },
            ))))
        } else if wiz.aborted() {
            Some(Transition::Pop)
        } else {
            None
        }
    }))
}

fn make_change_traffic(btn: ScreenRectangle) -> Box<dyn State> {
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
        app.primary.clear_sim();
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
