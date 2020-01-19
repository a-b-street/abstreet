use crate::edit::EditMode;
use crate::game::{State, Transition, WizardState};
use crate::helpers::ID;
use crate::managed::Composite;
use crate::sandbox::gameplay::{change_scenario, spawner, GameplayMode, GameplayState};
use crate::sandbox::overlays::Overlays;
use crate::sandbox::SandboxMode;
use crate::ui::UI;
use ezgui::{
    hotkey, lctrl, Color, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line,
    ManagedWidget, Text, VerticalAlignment,
};
use geom::Polygon;
use map_model::IntersectionID;
use std::collections::BTreeSet;

// TODO Maybe remember what things were spawned, offer to replay this later
pub struct Freeform {
    // TODO Clean these up later when done?
    pub spawn_pts: BTreeSet<IntersectionID>,
}

impl Freeform {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> (Composite, Box<dyn GameplayState>) {
        (
            freeform_controller(ctx, ui, GameplayMode::Freeform, "none"),
            Box::new(Freeform {
                spawn_pts: BTreeSet::new(),
            }),
        )
    }
}

impl GameplayState for Freeform {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI, _: &mut Overlays) -> Option<Transition> {
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
                ManagedWidget::draw_text(ctx, Text::from(Line("Sandbox").size(26))).margin(5),
                ManagedWidget::draw_batch(
                    ctx,
                    GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 50.0))]),
                )
                .margin(5),
                ManagedWidget::draw_text(ctx, Text::from(Line("Map:").size(18).roboto_bold()))
                    .margin(5),
                // TODO Different button style
                Composite::detailed_text_button(
                    ctx,
                    Text::from(
                        Line(format!("{} ▼", ui.primary.map.get_name()))
                            .fg(Color::BLACK)
                            .size(18)
                            .roboto(),
                    ),
                    lctrl(Key::L),
                    "change map",
                )
                .margin(5),
                ManagedWidget::draw_text(ctx, Text::from(Line("Traffic:").size(18).roboto_bold()))
                    .margin(5),
                Composite::detailed_text_button(
                    ctx,
                    Text::from(
                        Line(format!("{} ▼", scenario_name))
                            .fg(Color::BLACK)
                            .size(18)
                            .roboto(),
                    ),
                    hotkey(Key::S),
                    "change scenario",
                )
                .margin(5),
                Composite::svg_button(ctx, "assets/tools/edit_map.svg", "edit map", lctrl(Key::E))
                    .margin(5),
            ])
            .centered()
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
