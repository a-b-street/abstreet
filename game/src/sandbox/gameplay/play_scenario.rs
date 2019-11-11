use crate::game::{msg, Transition, WizardState};
use crate::sandbox::gameplay::{change_scenario, load_map, GameplayState};
use crate::sandbox::overlays::Overlays;
use crate::ui::UI;
use ezgui::{hotkey, lctrl, EventCtx, Key, ModalMenu};

pub struct PlayScenario;

impl PlayScenario {
    pub fn new(name: &String, ctx: &EventCtx) -> (ModalMenu, Box<dyn GameplayState>) {
        (
            ModalMenu::new(
                &format!("Playing {}", name),
                vec![
                    (hotkey(Key::S), "start another scenario"),
                    (lctrl(Key::L), "load another map"),
                    (hotkey(Key::H), "help"),
                ],
                ctx,
            ),
            Box::new(PlayScenario),
        )
    }
}

impl GameplayState for PlayScenario {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        _: &mut UI,
        _: &mut Overlays,
        menu: &mut ModalMenu,
    ) -> Option<Transition> {
        menu.event(ctx);
        if menu.action("start another scenario") {
            return Some(Transition::Push(WizardState::new(Box::new(
                change_scenario,
            ))));
        }
        if menu.action("load another map") {
            return Some(Transition::Push(WizardState::new(Box::new(load_map))));
        }
        if menu.action("help") {
            return Some(Transition::Push(msg(
                "Help",
                vec![
                    "Do things seem a bit quiet?",
                    "The simulation starts at midnight, so you might need to wait a bit.",
                    "Try using the speed controls on the left.",
                ],
            )));
        }
        None
    }
}
