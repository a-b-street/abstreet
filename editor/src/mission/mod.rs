mod all_trips;
mod dataviz;
mod individ_trips;
mod neighborhood;
mod scenario;

use crate::game::{State, Transition, WizardState};
use crate::sandbox::SandboxMode;
use crate::ui::UI;
use abstutil::Timer;
use ezgui::{hotkey, EventCtx, GfxCtx, Key, ModalMenu, Wizard, WrappedWizard};
use geom::Duration;
use popdat::trips_to_scenario;
use sim::Scenario;

pub struct MissionEditMode {
    menu: ModalMenu,
}

impl MissionEditMode {
    pub fn new(ctx: &EventCtx, ui: &mut UI) -> MissionEditMode {
        // TODO Warn first?
        ui.primary.reset_sim();

        MissionEditMode {
            menu: ModalMenu::new(
                "Mission Edit Mode",
                vec![
                    vec![
                        (hotkey(Key::D), "visualize population data"),
                        (hotkey(Key::T), "visualize individual PSRC trips"),
                        (hotkey(Key::A), "visualize all PSRC trips"),
                    ],
                    vec![
                        (hotkey(Key::S), "set up simulation with PSRC trips"),
                        (hotkey(Key::Q), "create scenario from PSRC trips"),
                        (hotkey(Key::N), "manage neighborhoods"),
                        (hotkey(Key::W), "load scenario"),
                        (None, "create new scenario"),
                    ],
                    vec![(hotkey(Key::Escape), "quit")],
                ],
                ctx,
            ),
        }
    }
}

impl State for MissionEditMode {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        self.menu.handle_event(ctx, None);
        ctx.canvas.handle_event(ctx.input);

        if self.menu.action("quit") {
            return Transition::Pop;
        } else if self.menu.action("visualize population data") {
            return Transition::Push(Box::new(dataviz::DataVisualizer::new(ctx, ui)));
        } else if self.menu.action("visualize individual PSRC trips") {
            return Transition::Push(Box::new(individ_trips::TripsVisualizer::new(ctx, ui)));
        } else if self.menu.action("visualize all PSRC trips") {
            return Transition::Push(Box::new(all_trips::TripsVisualizer::new(ctx, ui)));
        } else if self.menu.action("set up simulation with PSRC trips") {
            ctx.loading_screen("setup PSRC scenario", |_, mut timer| {
                let scenario = trips_to_scenario(
                    &ui.primary.map,
                    Duration::ZERO,
                    Duration::END_OF_DAY,
                    &mut timer,
                );
                scenario.instantiate(
                    &mut ui.primary.sim,
                    &ui.primary.map,
                    &mut ui.primary.current_flags.sim_flags.make_rng(),
                    timer,
                );
                ui.primary
                    .sim
                    .step(&ui.primary.map, Duration::const_seconds(0.1));
            });
            return Transition::Replace(Box::new(SandboxMode::new(ctx)));
        } else if self.menu.action("create scenario from PSRC trips") {
            return Transition::Push(WizardState::new(Box::new(convert_trips_to_scenario)));
        } else if self.menu.action("manage neighborhoods") {
            return Transition::Push(Box::new(neighborhood::NeighborhoodPicker::new()));
        } else if self.menu.action("load scenario") {
            return Transition::Push(WizardState::new(Box::new(load_scenario)));
        } else if self.menu.action("create new scenario") {
            return Transition::Push(WizardState::new(Box::new(create_new_scenario)));
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.menu.draw(g);
    }
}

fn convert_trips_to_scenario(
    wiz: &mut Wizard,
    ctx: &mut EventCtx,
    ui: &mut UI,
) -> Option<Transition> {
    let (t1, t2) = pick_time_range(
        &mut wiz.wrap(ctx),
        "Include trips departing AFTER when?",
        "Include trips departing BEFORE when?",
    )?;
    ctx.loading_screen("extract PSRC scenario", |_, mut timer| {
        trips_to_scenario(&ui.primary.map, t1, t2, &mut timer).save();
    });
    Some(Transition::Pop)
}

fn load_scenario(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let map_name = ui.primary.map.get_name().to_string();
    let s = wiz.wrap(ctx).choose_string("Load which scenario?", || {
        abstutil::list_all_objects(abstutil::SCENARIOS, &map_name)
    })?;
    let scenario = abstutil::read_binary(
        &abstutil::path1_bin(&map_name, abstutil::SCENARIOS, &s),
        &mut Timer::throwaway(),
    )
    .unwrap();
    Some(Transition::Replace(Box::new(
        scenario::ScenarioManager::new(scenario, ctx),
    )))
}

fn create_new_scenario(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let name = wiz.wrap(ctx).input_string("Name the scenario")?;
    Some(Transition::Replace(Box::new(
        scenario::ScenarioManager::new(
            Scenario {
                scenario_name: name,
                map_name: ui.primary.map.get_name().to_string(),
                seed_parked_cars: Vec::new(),
                spawn_over_time: Vec::new(),
                border_spawn_over_time: Vec::new(),
                individ_trips: Vec::new(),
            },
            ctx,
        ),
    )))
}

pub fn pick_time_range(
    wizard: &mut WrappedWizard,
    low_query: &str,
    high_query: &str,
) -> Option<(Duration, Duration)> {
    let t1 = wizard.input_time_slider(low_query, Duration::ZERO, Duration::END_OF_DAY)?;
    let t2 = wizard.input_time_slider(high_query, t1, Duration::END_OF_DAY)?;
    Some((t1, t2))
}
