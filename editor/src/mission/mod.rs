mod all_trips;
mod dataviz;
mod individ_trips;
mod neighborhood;
mod scenario;

use crate::game::{GameState, Mode};
use crate::helpers::ID;
use crate::render::DrawOptions;
use crate::ui::ShowEverything;
use crate::ui::UI;
use abstutil::Timer;
use ezgui::{EventCtx, EventLoopMode, GfxCtx, Key, ModalMenu, Wizard};
use geom::{Circle, Distance, Duration, Pt2D};
use map_model::BuildingID;

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
}

impl MissionEditMode {
    pub fn new(ctx: &EventCtx, ui: &mut UI) -> MissionEditMode {
        // TODO Warn first?
        ui.primary.reset_sim();

        MissionEditMode {
            state: State::Exploring(ModalMenu::new(
                "Mission Edit Mode",
                vec![
                    (Some(Key::Escape), "quit"),
                    (Some(Key::D), "visualize population data"),
                    (Some(Key::T), "visualize individual trips"),
                    (Some(Key::A), "visualize all trips"),
                    (Some(Key::N), "manage neighborhoods"),
                    (Some(Key::W), "manage scenarios"),
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
                        } else if menu.action("visualize individual trips") {
                            mode.state = State::IndividualTrips(
                                individ_trips::TripsVisualizer::new(ctx, &state.ui),
                            );
                        } else if menu.action("visualize all trips") {
                            mode.state =
                                State::AllTrips(all_trips::TripsVisualizer::new(ctx, &state.ui));
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
                        if viz.event(ctx, &mut state.ui) {
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

pub struct Trip {
    pub from: BuildingID,
    pub to: BuildingID,
    pub depart_at: Duration,
    pub purpose: (popdat::psrc::Purpose, popdat::psrc::Purpose),
    pub mode: popdat::psrc::Mode,
    pub trip_time: Duration,
    pub trip_dist: Distance,
}

impl Trip {
    pub fn end_time(&self) -> Duration {
        self.depart_at + self.trip_time
    }
}

pub fn clip_trips(popdat: &popdat::PopDat, ui: &UI, timer: &mut Timer) -> Vec<Trip> {
    let mut results = Vec::new();
    let bounds = ui.primary.map.get_gps_bounds();
    timer.start_iter("clip trips", popdat.trips.len());
    for trip in &popdat.trips {
        timer.next();
        if !bounds.contains(trip.from) || !bounds.contains(trip.to) {
            continue;
        }
        let from = find_building_containing(Pt2D::from_gps(trip.from, bounds).unwrap(), ui);
        let to = find_building_containing(Pt2D::from_gps(trip.to, bounds).unwrap(), ui);
        if from.is_some() && to.is_some() {
            results.push(Trip {
                from: from.unwrap(),
                to: to.unwrap(),
                depart_at: trip.depart_at,
                purpose: trip.purpose,
                mode: trip.mode,
                trip_time: trip.trip_time,
                trip_dist: trip.trip_dist,
            });
        }
    }
    results
}

fn find_building_containing(pt: Pt2D, ui: &UI) -> Option<BuildingID> {
    for obj in ui
        .primary
        .draw_map
        .get_matching_objects(Circle::new(pt, Distance::meters(3.0)).get_bounds())
    {
        if let ID::Building(b) = obj {
            if ui.primary.map.get_b(b).polygon.contains_pt(pt) {
                return Some(b);
            }
        }
    }
    None
}
