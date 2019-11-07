use crate::game::{msg, Transition, WizardState};
use crate::sandbox::gameplay::{change_scenario, load_map, GameplayState};
use crate::sandbox::overlays::Overlays;
use crate::sandbox::spawner;
use crate::ui::UI;
use ezgui::{hotkey, lctrl, EventCtx, Key, ModalMenu};
use sim::Analytics;

// TODO Maybe remember what things were spawned, offer to replay this later
pub struct Freeform;

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
            Box::new(Freeform),
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
        _: &Analytics,
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
}
