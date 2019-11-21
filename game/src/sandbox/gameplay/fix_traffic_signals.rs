use crate::game::{msg, Transition};
use crate::sandbox::gameplay::faster_trips::faster_trips_panel;
use crate::sandbox::gameplay::{manage_overlays, GameplayState};
use crate::sandbox::overlays::{calculate_intersection_delay, Overlays};
use crate::ui::UI;
use ezgui::{hotkey, EventCtx, Key, ModalMenu};
use geom::Duration;
use sim::TripMode;

pub struct FixTrafficSignals {
    time: Duration,
}

impl FixTrafficSignals {
    pub fn new(ctx: &EventCtx) -> (ModalMenu, Box<dyn GameplayState>) {
        (
            ModalMenu::new(
                "Fix traffic signals",
                vec![
                    (hotkey(Key::F), "find slowest traffic signals"),
                    (hotkey(Key::H), "help"),
                ],
                ctx,
            ),
            Box::new(FixTrafficSignals {
                time: Duration::ZERO,
            }),
        )
    }
}

impl GameplayState for FixTrafficSignals {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        overlays: &mut Overlays,
        menu: &mut ModalMenu,
    ) -> Option<Transition> {
        menu.event(ctx);

        // Technically this shows stop signs too, but mostly the bottlenecks are signals.
        if manage_overlays(
            menu,
            ctx,
            "find slowest traffic signals",
            "hide slowest traffic signals",
            overlays,
            match overlays {
                Overlays::IntersectionDelay(_, _) => true,
                _ => false,
            },
            self.time != ui.primary.sim.time(),
        ) {
            *overlays = Overlays::IntersectionDelay(
                ui.primary.sim.time(),
                calculate_intersection_delay(ctx, ui),
            );
        }

        if self.time != ui.primary.sim.time() {
            self.time = ui.primary.sim.time();
            menu.set_info(ctx, faster_trips_panel(TripMode::Drive, ui));
        }

        if menu.action("help") {
            return Some(Transition::Push(msg(
                "Help",
                vec![
                    "All of the traffic signals follow one timing plan through the whole day.",
                    "(Due to budget cuts, none of the vehicle-actuated signals are working -- don't worry if you don't know what these are.)",
                ])));
        }
        None
    }
}
