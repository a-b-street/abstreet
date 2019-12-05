use crate::game::{msg, Transition, WizardState};
use crate::helpers::ID;
use crate::sandbox::gameplay::{change_scenario, load_map, spawner, GameplayState};
use crate::sandbox::overlays::Overlays;
use crate::ui::UI;
use ezgui::{hotkey, lctrl, Color, EventCtx, GfxCtx, Key, Line, ModalMenu, Text};
use map_model::IntersectionID;
use sim::Analytics;
use std::collections::BTreeSet;

// TODO Maybe remember what things were spawned, offer to replay this later
pub struct Freeform {
    // TODO Clean these up later when done?
    pub spawn_pts: BTreeSet<IntersectionID>,
}

impl Freeform {
    pub fn new(ctx: &EventCtx) -> (ModalMenu, Box<dyn GameplayState>) {
        (
            ModalMenu::new(
                "Freeform mode",
                vec![
                    (hotkey(Key::S), "start a scenario"),
                    (lctrl(Key::L), "load another map"),
                    (hotkey(Key::H), "help"),
                ],
                ctx,
            ),
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
        _: &Analytics,
        menu: &mut ModalMenu,
    ) -> Option<Transition> {
        menu.event(ctx);
        if menu.action("start a scenario") {
            return Some(Transition::Push(WizardState::new(Box::new(
                change_scenario,
            ))));
        }
        if menu.action("load another map") {
            return Some(Transition::Push(WizardState::new(Box::new(load_map))));
        }
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
                let mut txt = Text::new();
                for line in cnt.describe() {
                    txt.add(Line(line));
                }
                g.draw_mouse_tooltip(&txt);
            }
        }
    }
}
