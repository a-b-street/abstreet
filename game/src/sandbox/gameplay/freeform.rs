use crate::app::App;
use crate::edit::EditMode;
use crate::game::{State, Transition, WizardState};
use crate::helpers::nice_map_name;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::SandboxControls;
use crate::sandbox::SandboxMode;
use ezgui::{
    hotkey, lctrl, Btn, Choice, Color, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment,
    Key, Line, ScreenRectangle, TextExt, VerticalAlignment, Widget,
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
    ) -> Option<Transition> {
        match self.top_center.event(ctx, app) {
            Some(WrappedOutcome::Transition(t)) => {
                return Some(t);
            }
            Some(WrappedOutcome::Clicked(_)) => unreachable!(),
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
) -> WrappedComposite {
    let c = Composite::new(
        Widget::row(vec![
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
        .centered()
        .bg(app.cs.panel_bg),
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
                        if n == "montlake" || n == "23rd" || n == "lakeslice" {
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
                    // Try to load a scenario with the same name exists
                    GameplayMode::PlayScenario(_, ref scenario) => {
                        if abstutil::file_exists(abstutil::path_scenario(&name, scenario)) {
                            GameplayMode::PlayScenario(abstutil::path_map(&name), scenario.clone())
                        } else {
                            GameplayMode::Freeform(abstutil::path_map(&name))
                        }
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
