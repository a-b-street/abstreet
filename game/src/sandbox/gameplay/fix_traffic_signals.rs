use crate::common::Overlays;
use crate::game::Transition;
use crate::helpers::cmp_duration_shorter;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::sandbox::gameplay::{challenge_controller, FinalScore, GameplayMode, GameplayState};
use crate::sandbox::SandboxControls;
use crate::ui::UI;
use ezgui::{Button, EventCtx, GfxCtx, Line, ManagedWidget, Text};
use geom::{Duration, Statistic, Time};
use map_model::{IntersectionID, Map};
use sim::{BorderSpawnOverTime, OriginDestination, Scenario};

const GOAL: Duration = Duration::const_seconds(30.0);

pub struct FixTrafficSignals {
    time: Time,
    once: bool,
    top_center: WrappedComposite,
    // TODO Keeping a copy in here seems redundant?
    mode: GameplayMode,
}

impl FixTrafficSignals {
    pub fn new(ctx: &mut EventCtx, ui: &UI, mode: GameplayMode) -> Box<dyn GameplayState> {
        Box::new(FixTrafficSignals {
            time: Time::START_OF_DAY,
            once: true,
            top_center: make_top_center(ctx, ui, mode.clone()),
            mode,
        })
    }
}

impl GameplayState for FixTrafficSignals {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        _: &mut SandboxControls,
    ) -> (Option<Transition>, bool) {
        // Once is never...
        if self.once {
            ui.overlay = Overlays::finished_trips_histogram(ctx, ui);
            self.once = false;
        }

        match self.top_center.event(ctx, ui) {
            Some(WrappedOutcome::Transition(t)) => {
                return (Some(t), false);
            }
            Some(WrappedOutcome::Clicked(_)) => unreachable!(),
            None => {}
        }
        if self.time != ui.primary.sim.time() {
            self.time = ui.primary.sim.time();
            self.top_center = make_top_center(ctx, ui, self.mode.clone());
        }

        if ui.primary.sim.is_done() {
            return (
                Some(Transition::Push(FinalScore::new(
                    ctx,
                    final_score(ui),
                    self.mode.clone(),
                ))),
                false,
            );
        }

        (None, false)
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.top_center.draw(g);
    }
}

fn make_top_center(ctx: &mut EventCtx, ui: &UI, mode: GameplayMode) -> WrappedComposite {
    let mut txt = Text::new();
    let (now, _, _) = ui
        .primary
        .sim
        .get_analytics()
        .all_finished_trips(ui.primary.sim.time());
    let (baseline, _, _) = ui.prebaked().all_finished_trips(ui.primary.sim.time());
    txt.add(Line("Average trip time: ").size(20));
    if now.count() > 0 && baseline.count() > 0 {
        txt.append_all(cmp_duration_shorter(
            now.select(Statistic::Mean),
            baseline.select(Statistic::Mean),
        ));
    } else {
        txt.append(Line("same as baseline"));
    }

    challenge_controller(
        ctx,
        mode,
        "Traffic Signals Challenge",
        vec![
            ManagedWidget::row(vec![
                ManagedWidget::draw_text(ctx, txt).margin(5),
                // TODO Should also recalculate if the overlay changes, but this is close enough
                match ui.overlay {
                    Overlays::FinishedTripsHistogram(_, _) => {
                        Button::inactive_btn(ctx, Text::from(Line("details").size(20)))
                    }
                    _ => WrappedComposite::nice_text_button(
                        ctx,
                        Text::from(Line("details").size(20)),
                        None,
                        "details",
                    ),
                }
                .align_right(),
            ]),
            ManagedWidget::draw_text(
                ctx,
                Text::from(Line(format!("Goal: {} faster", GOAL)).size(20)),
            )
            .margin(5),
        ],
    )
    .maybe_cb(
        "details",
        Box::new(|ctx, ui| {
            ui.overlay = Overlays::finished_trips_histogram(ctx, ui);
            None
        }),
    )
}

fn final_score(ui: &UI) -> String {
    let time = ui.primary.sim.time();
    let now = ui
        .primary
        .sim
        .get_analytics()
        .all_finished_trips(time)
        .0
        .select(Statistic::Mean);
    let baseline = ui
        .prebaked()
        .all_finished_trips(time)
        .0
        .select(Statistic::Mean);

    if now < baseline - GOAL {
        format!(
            "COMPLETED! Average trip time is now {}, which is {} faster than the baseline {}",
            now,
            baseline - now,
            baseline
        )
    } else if now < baseline {
        format!(
            "Almost there! Average trip time is now {}, which is {} faster than the baseline {}. \
             Can you reduce the average by {}?",
            now,
            baseline - now,
            baseline,
            GOAL
        )
    } else if now.epsilon_eq(baseline) {
        format!(
            "... Did you change anything? Average trip time is {}, same as the baseline",
            now
        )
    } else {
        format!(
            "Err... how did you make things WORSE?! Average trip time is {}, which is {} slower \
             than the baseline {}",
            now,
            now - baseline,
            baseline
        )
    }
}

// TODO Hacks in here, because I'm not convinced programatically specifying this is right. I think
// the Scenario abstractions and UI need to change to make this convenient to express in JSON / the
// UI.

// Motivate a separate left turn phase for north/south, but not left/right
pub fn tutorial_scenario_lvl1(map: &Map) -> Scenario {
    // TODO In lieu of the deleted labels
    let north = IntersectionID(2);
    let south = IntersectionID(3);
    // Hush, east/west is more cognitive overhead for me. >_<
    let left = IntersectionID(1);
    let right = IntersectionID(0);

    let mut s = Scenario::empty(map, "tutorial lvl1");

    // What's the essence of what I've specified below? Don't care about the time distribution,
    // exact number of agents, different modes. It's just an OD matrix with relative weights.
    //
    //        north  south  left  right
    // north   0      3      1     2
    // south   3      ... and so on
    // left
    // right
    //
    // The table isn't super easy to grok. But it motivates the UI for entering this info:
    //
    // 1) Select all of the sources
    // 2) Select all of the sinks (option to use the same set)
    // 3) For each (src, sink) pair, ask (none, light, medium, heavy)

    // Arterial straight
    heavy(&mut s, map, south, north);
    heavy(&mut s, map, north, south);
    // Arterial left turns
    medium(&mut s, map, south, left);
    medium(&mut s, map, north, right);
    // Arterial right turns
    light(&mut s, map, south, right);
    light(&mut s, map, north, left);

    // Secondary straight
    medium(&mut s, map, left, right);
    medium(&mut s, map, right, left);
    // Secondary right turns
    medium(&mut s, map, left, south);
    medium(&mut s, map, right, north);
    // Secondary left turns
    light(&mut s, map, left, north);
    light(&mut s, map, right, south);

    s
}

// Motivate a pedestrian scramble cycle
pub fn tutorial_scenario_lvl2(map: &Map) -> Scenario {
    let north = IntersectionID(3);
    let south = IntersectionID(3);
    let left = IntersectionID(1);
    let right = IntersectionID(0);

    let mut s = tutorial_scenario_lvl1(map);
    s.scenario_name = "tutorial lvl2".to_string();

    // TODO The first few phases aren't affected, because the peds walk slowly from the border.
    // Start them from a building instead?
    // TODO All the peds get through in a single wave; spawn them continuously?
    // TODO The metrics shown are just for driving trips...
    heavy_peds(&mut s, map, south, north);
    heavy_peds(&mut s, map, north, south);
    heavy_peds(&mut s, map, left, right);
    heavy_peds(&mut s, map, right, left);

    s
}

fn heavy(s: &mut Scenario, map: &Map, from: IntersectionID, to: IntersectionID) {
    spawn(s, map, from, to, 100, 0);
}
fn heavy_peds(s: &mut Scenario, map: &Map, from: IntersectionID, to: IntersectionID) {
    spawn(s, map, from, to, 0, 100);
}
fn medium(s: &mut Scenario, map: &Map, from: IntersectionID, to: IntersectionID) {
    spawn(s, map, from, to, 100, 0);
}
fn light(s: &mut Scenario, map: &Map, from: IntersectionID, to: IntersectionID) {
    spawn(s, map, from, to, 100, 0);
}

fn spawn(
    s: &mut Scenario,
    map: &Map,
    from: IntersectionID,
    to: IntersectionID,
    num_cars: usize,
    num_peds: usize,
) {
    s.border_spawn_over_time.push(BorderSpawnOverTime {
        num_peds,
        num_cars,
        num_bikes: 0,
        percent_use_transit: 0.0,
        start_time: Time::START_OF_DAY,
        stop_time: Time::START_OF_DAY + Duration::minutes(5),
        start_from_border: map.get_i(from).some_outgoing_road(map),
        goal: OriginDestination::EndOfRoad(map.get_i(to).some_incoming_road(map)),
    });
}
