mod all_trips;
mod individ_trips;
mod neighborhood;
mod scenario;

use crate::game::{State, Transition, WizardState};
use crate::ui::UI;
use abstutil::Timer;
use ezgui::{hotkey, EventCtx, GfxCtx, Key, ModalMenu, Wizard};

pub struct MissionEditMode {
    menu: ModalMenu,
}

impl MissionEditMode {
    pub fn new(ctx: &EventCtx) -> MissionEditMode {
        MissionEditMode {
            menu: ModalMenu::new(
                "Mission Edit Mode",
                vec![
                    (hotkey(Key::T), "visualize individual PSRC trips"),
                    (hotkey(Key::A), "visualize all PSRC trips"),
                    (hotkey(Key::N), "manage neighborhoods"),
                    (hotkey(Key::W), "load scenario"),
                    (hotkey(Key::Escape), "quit"),
                ],
                ctx,
            ),
        }
    }
}

impl State for MissionEditMode {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        self.menu.event(ctx);
        ctx.canvas_movement();

        if self.menu.action("quit") {
            return Transition::Pop;
        } else if self.menu.action("visualize individual PSRC trips") {
            return Transition::Push(Box::new(individ_trips::TripsVisualizer::new(ctx, ui)));
        } else if self.menu.action("visualize all PSRC trips") {
            return Transition::Push(Box::new(all_trips::TripsVisualizer::new(ctx, ui)));
        } else if self.menu.action("manage neighborhoods") {
            return Transition::Push(Box::new(neighborhood::NeighborhoodPicker::new()));
        } else if self.menu.action("load scenario") {
            return Transition::Push(WizardState::new(Box::new(load_scenario)));
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.menu.draw(g);
    }
}

fn load_scenario(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let map_name = ui.primary.map.get_name().to_string();
    let s = wiz.wrap(ctx).choose_string("Load which scenario?", || {
        abstutil::list_all_objects(abstutil::path_all_scenarios(&map_name))
    })?;
    let scenario = abstutil::read_binary(
        abstutil::path_scenario(&map_name, &s),
        &mut Timer::throwaway(),
    );
    Some(Transition::Replace(Box::new(
        scenario::ScenarioManager::new(scenario, ctx, ui),
    )))
}
