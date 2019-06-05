mod all_trips;
mod dataviz;
mod individ_trips;
mod neighborhood;
mod scenario;
mod trips;

use self::trips::{pick_time_range, trips_to_scenario};
use crate::game::{GameState, Mode};
use crate::render::DrawOptions;
use crate::sandbox::SandboxMode;
use crate::ui::ShowEverything;
use crate::ui::UI;
use ezgui::{hotkey, EventCtx, EventLoopMode, GfxCtx, Key, ModalMenu, Wizard, WrappedWizard};
use geom::Duration;

pub struct MissionEditMode {
    state: State,
}

enum State {
    Exploring(ModalMenu),
    Neighborhood(neighborhood::NeighborhoodEditor),
    Scenario(scenario::ScenarioEditor),
    DataViz(dataviz::DataVisualizer),
    IndividualTrips(individ_trips::TripsVisualizer),
    AllTrips(all_trips::TripsVisualizer),
    TripsToScenario(Wizard),
}

impl MissionEditMode {
    pub fn new(ctx: &EventCtx, ui: &mut UI) -> MissionEditMode {
        // TODO Warn first?
        ui.primary.reset_sim();

        MissionEditMode {
            state: State::Exploring(ModalMenu::new(
                "Mission Edit Mode",
                vec![
                    (hotkey(Key::Escape), "quit"),
                    (hotkey(Key::D), "visualize population data"),
                    (hotkey(Key::T), "visualize individual PSRC trips"),
                    (hotkey(Key::A), "visualize all PSRC trips"),
                    (hotkey(Key::S), "set up simulation with PSRC trips"),
                    (hotkey(Key::Q), "create scenario from PSRC trips"),
                    (hotkey(Key::N), "manage neighborhoods"),
                    (hotkey(Key::W), "manage scenarios"),
                ],
                ctx,
            )),
        }
    }

    pub fn event(state: &mut GameState, ctx: &mut EventCtx) -> EventLoopMode {
        match state.mode {
            Mode::Mission(ref mut mode) => {
                match mode.state {
                    State::Exploring(ref mut menu) => {
                        menu.handle_event(ctx, None);
                        ctx.canvas.handle_event(ctx.input);

                        if menu.action("quit") {
                            state.mode = Mode::SplashScreen(Wizard::new(), None);
                        } else if menu.action("visualize population data") {
                            mode.state =
                                State::DataViz(dataviz::DataVisualizer::new(ctx, &state.ui));
                        } else if menu.action("visualize individual PSRC trips") {
                            mode.state = State::IndividualTrips(
                                individ_trips::TripsVisualizer::new(ctx, &state.ui),
                            );
                        } else if menu.action("visualize all PSRC trips") {
                            mode.state =
                                State::AllTrips(all_trips::TripsVisualizer::new(ctx, &state.ui));
                        } else if menu.action("set up simulation with PSRC trips") {
                            let scenario = trips_to_scenario(
                                ctx,
                                &state.ui,
                                Duration::ZERO,
                                Duration::parse("23:59:59.9").unwrap(),
                            );
                            ctx.loading_screen("instantiate scenario", |_, timer| {
                                scenario.instantiate(
                                    &mut state.ui.primary.sim,
                                    &state.ui.primary.map,
                                    &mut state.ui.primary.current_flags.sim_flags.make_rng(),
                                    timer,
                                );
                                state
                                    .ui
                                    .primary
                                    .sim
                                    .step(&state.ui.primary.map, Duration::const_seconds(0.1));
                            });
                            state.mode = Mode::Sandbox(SandboxMode::new(ctx));
                        } else if menu.action("create scenario from PSRC trips") {
                            mode.state = State::TripsToScenario(Wizard::new());
                        } else if menu.action("manage neighborhoods") {
                            mode.state = State::Neighborhood(
                                neighborhood::NeighborhoodEditor::PickNeighborhood(Wizard::new()),
                            );
                        } else if menu.action("manage scenarios") {
                            mode.state = State::Scenario(scenario::ScenarioEditor::PickScenario(
                                Wizard::new(),
                            ));
                        }
                    }
                    State::DataViz(ref mut viz) => {
                        if viz.event(ctx, &state.ui) {
                            mode.state = MissionEditMode::new(ctx, &mut state.ui).state;
                        }
                    }
                    State::IndividualTrips(ref mut viz) => {
                        if viz.event(ctx, &mut state.ui) {
                            mode.state = MissionEditMode::new(ctx, &mut state.ui).state;
                        }
                    }
                    State::AllTrips(ref mut viz) => {
                        if let Some(evmode) = viz.event(ctx, &mut state.ui) {
                            return evmode;
                        } else {
                            mode.state = MissionEditMode::new(ctx, &mut state.ui).state;
                        }
                    }
                    State::TripsToScenario(ref mut wizard) => {
                        if let Some((t1, t2)) =
                            pick_time_range(wizard.wrap(&mut ctx.input, ctx.canvas))
                        {
                            trips_to_scenario(ctx, &state.ui, t1, t2).save();
                            mode.state = MissionEditMode::new(ctx, &mut state.ui).state;
                        } else if wizard.aborted() {
                            mode.state = MissionEditMode::new(ctx, &mut state.ui).state;
                        }
                    }
                    State::Neighborhood(ref mut editor) => {
                        if editor.event(ctx, &state.ui) {
                            mode.state = MissionEditMode::new(ctx, &mut state.ui).state;
                        }
                    }
                    State::Scenario(ref mut editor) => {
                        if let Some(new_mode) = editor.event(ctx, &mut state.ui) {
                            state.mode = new_mode;
                        }
                    }
                }
                EventLoopMode::InputOnly
            }
            _ => unreachable!(),
        }
    }

    pub fn draw(state: &GameState, g: &mut GfxCtx) {
        state.ui.draw(
            g,
            DrawOptions::new(),
            &state.ui.primary.sim,
            &ShowEverything::new(),
        );

        match state.mode {
            Mode::Mission(ref mode) => match mode.state {
                State::Exploring(ref menu) => {
                    menu.draw(g);
                }
                State::DataViz(ref viz) => {
                    viz.draw(g, &state.ui);
                }
                State::IndividualTrips(ref viz) => {
                    viz.draw(g, &state.ui);
                }
                State::AllTrips(ref viz) => {
                    viz.draw(g, &state.ui);
                }
                State::TripsToScenario(ref wizard) => {
                    wizard.draw(g);
                }
                State::Neighborhood(ref editor) => {
                    editor.draw(g, &state.ui);
                }
                State::Scenario(ref editor) => {
                    editor.draw(g, &state.ui);
                }
            },
            _ => unreachable!(),
        }
    }
}

pub fn input_time(wizard: &mut WrappedWizard, query: &str) -> Option<Duration> {
    wizard.input_something(query, None, Box::new(|line| Duration::parse(&line)))
}
