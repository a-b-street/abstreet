use crate::game::{msg, Transition};
use crate::managed::Composite;
use crate::sandbox::gameplay::freeform::freeform_controller;
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::overlays::Overlays;
use crate::ui::UI;
use ezgui::{hotkey, EventCtx, Key, ModalMenu};

pub struct PlayScenario;

impl PlayScenario {
    pub fn new(
        name: &String,
        ctx: &mut EventCtx,
        ui: &UI,
    ) -> (ModalMenu, Composite, Box<dyn GameplayState>) {
        (
            ModalMenu::new(
                format!("Playing {}", name),
                vec![(hotkey(Key::H), "help")],
                ctx,
            ),
            freeform_controller(ctx, ui, GameplayMode::PlayScenario(name.to_string()), name),
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
