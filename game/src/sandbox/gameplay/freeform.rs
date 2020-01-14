use crate::edit::EditMode;
use crate::game::{msg, State, Transition, WizardState};
use crate::helpers::ID;
use crate::managed::Composite;
use crate::sandbox::gameplay::{change_scenario, spawner, GameplayMode, GameplayState};
use crate::sandbox::overlays::Overlays;
use crate::sandbox::SandboxMode;
use crate::ui::UI;
use ezgui::{
    hotkey, lctrl, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, ManagedWidget,
    ModalMenu, Text, VerticalAlignment,
};
use map_model::IntersectionID;
use std::collections::BTreeSet;

// TODO Maybe remember what things were spawned, offer to replay this later
pub struct Freeform {
    // TODO Clean these up later when done?
    pub spawn_pts: BTreeSet<IntersectionID>,
}

impl Freeform {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> (ModalMenu, Composite, Box<dyn GameplayState>) {
        (
            ModalMenu::new("Freeform mode", vec![(hotkey(Key::H), "help")], ctx),
            freeform_controller(ctx, ui, GameplayMode::Freeform, "empty scenario"),
            Box::new(Freeform {
                spawn_pts: BTreeSet::new(),
            }),
        )
    }
}

impl GameplayState for Freeform {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        _: &mut Overlays,
        menu: &mut ModalMenu,
    ) -> Option<Transition> {
        menu.event(ctx);
        if menu.action("help") {
            return Some(Transition::Push(msg("Help", vec!["This simulation is empty by default.", "Try right-clicking an intersection and choosing to spawn agents (or just hover over it and press Z).", "You can also spawn agents from buildings or lanes.", "You can also start a full scenario to get realistic traffic."])));
        }
        if let Some(new_state) = spawner::AgentSpawner::new(ctx, ui) {
            return Some(Transition::Push(new_state));
        }
        if let Some(new_state) = spawner::SpawnManyAgents::new(ctx, ui) {
            return Some(Transition::Push(new_state));
        }
        None
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        // TODO Overriding draw options would be ideal, but...
        for i in &self.spawn_pts {
            g.draw_polygon(Color::GREEN.alpha(0.8), &ui.primary.map.get_i(*i).polygon);
        }

        if let Some(ID::Intersection(i)) = ui.primary.current_selection {
            if self.spawn_pts.contains(&i) {
                let cnt = ui.primary.sim.count_trips_involving_border(i);
                let mut txt = Text::new().with_bg();
                for line in cnt.describe() {
                    txt.add(Line(line));
                }
                g.draw_mouse_tooltip(&txt);
            }
        }
    }
}

pub fn freeform_controller(
    ctx: &mut EventCtx,
    ui: &UI,
    gameplay: GameplayMode,
    scenario_name: &str,
) -> Composite {
    Composite::new(
        ezgui::Composite::new(
            ManagedWidget::row(vec![
                ManagedWidget::col(vec![
                    Composite::text_button(ctx, "change map", lctrl(Key::L)),
                    ManagedWidget::draw_text(ctx, Text::from(Line(ui.primary.map.get_name()))),
                ]),
                ManagedWidget::col(vec![
                    Composite::text_button(ctx, "change scenario", hotkey(Key::S)),
                    ManagedWidget::draw_text(ctx, Text::from(Line(scenario_name))),
                ]),
                // TODO Refactor
                ManagedWidget::col(vec![
                    Composite::svg_button(
                        ctx,
                        "assets/tools/edit_map.svg",
                        "edit map",
                        lctrl(Key::E),
                    ),
                    {
                        let edits = ui.primary.map.get_edits();
                        let mut txt = Text::from(Line(&edits.edits_name));
                        if edits.dirty {
                            txt.append(Line("*"));
                        }
                        ManagedWidget::draw_text(ctx, txt)
                    },
                ]),
            ])
            .bg(Color::grey(0.4)),
        )
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx),
    )
    .cb("change map", {
        let gameplay = gameplay.clone();
        Box::new(move |_, _| Some(Transition::Push(make_load_map(gameplay.clone()))))
    })
    .cb(
        "change scenario",
        Box::new(|_, _| {
            Some(Transition::Push(WizardState::new(Box::new(
                change_scenario,
            ))))
        }),
    )
    .cb(
        "edit map",
        Box::new(move |ctx, ui| {
            Some(Transition::Replace(Box::new(EditMode::new(
                ctx,
                ui,
                gameplay.clone(),
            ))))
        }),
    )
}

fn make_load_map(gameplay: GameplayMode) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, ui| {
        if let Some(name) = wiz.wrap(ctx).choose_string("Load which map?", || {
            let current_map = ui.primary.map.get_name();
            abstutil::list_all_objects(abstutil::path_all_maps())
                .into_iter()
                .filter(|n| n != current_map)
                .collect()
        }) {
            ui.switch_map(ctx, abstutil::path_map(&name));
            // Assume a scenario with the same name exists.
            Some(Transition::PopThenReplace(Box::new(SandboxMode::new(
                ctx,
                ui,
                gameplay.clone(),
            ))))
        } else if wiz.aborted() {
            Some(Transition::Pop)
        } else {
            None
        }
    }))
}
