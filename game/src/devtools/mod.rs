mod all_trips;
mod individ_trips;
mod neighborhood;
mod scenario;

use crate::game::{State, Transition, WizardState};
use crate::managed::{ManagedGUIState, WrappedComposite};
use crate::ui::UI;
use abstutil::Timer;
use ezgui::{hotkey, EventCtx, Key, Wizard};

pub struct DevToolsMode;

impl DevToolsMode {
    pub fn new(ctx: &mut EventCtx) -> Box<dyn State> {
        ManagedGUIState::over_map(
            WrappedComposite::new(WrappedComposite::quick_menu(
                ctx,
                "Internal dev tools",
                vec![],
                vec![
                    (hotkey(Key::T), "visualize individual PSRC trips"),
                    (hotkey(Key::A), "visualize all PSRC trips"),
                    (hotkey(Key::N), "manage neighborhoods"),
                    (hotkey(Key::W), "load scenario"),
                ],
            ))
            .cb("X", Box::new(|_, _| Some(Transition::Pop)))
            .cb(
                "visualize individual PSRC trips",
                Box::new(|ctx, ui| {
                    Some(Transition::Push(Box::new(
                        individ_trips::TripsVisualizer::new(ctx, ui),
                    )))
                }),
            )
            .cb(
                "visualize all PSRC trips",
                Box::new(|ctx, ui| {
                    Some(Transition::Push(Box::new(all_trips::TripsVisualizer::new(
                        ctx, ui,
                    ))))
                }),
            )
            .cb(
                "manage neighborhoods",
                Box::new(|_, _| {
                    Some(Transition::Push(Box::new(
                        neighborhood::NeighborhoodPicker::new(),
                    )))
                }),
            )
            .cb(
                "load scenario",
                Box::new(|_, _| Some(Transition::Push(WizardState::new(Box::new(load_scenario))))),
            ),
        )
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
