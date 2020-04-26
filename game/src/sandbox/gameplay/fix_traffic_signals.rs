use crate::app::App;
use crate::game::Transition;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::sandbox::gameplay::{challenge_controller, GameplayMode, GameplayState};
use crate::sandbox::SandboxControls;
use ezgui::{EventCtx, GfxCtx};
use geom::{Duration, Time};
use map_model::{IntersectionID, Map};
use sim::{BorderSpawnOverTime, OriginDestination, ScenarioGenerator};

pub struct FixTrafficSignals {
    top_center: WrappedComposite,
}

impl FixTrafficSignals {
    pub fn new(ctx: &mut EventCtx, app: &App, mode: GameplayMode) -> Box<dyn GameplayState> {
        Box::new(FixTrafficSignals {
            top_center: challenge_controller(
                ctx,
                app,
                mode,
                "Traffic Signals Challenge",
                Vec::new(),
            ),
        })
    }
}

impl GameplayState for FixTrafficSignals {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        _: &mut SandboxControls,
    ) -> (Option<Transition>, bool) {
        match self.top_center.event(ctx, app) {
            Some(WrappedOutcome::Transition(t)) => {
                return (Some(t), false);
            }
            Some(WrappedOutcome::Clicked(_)) => unreachable!(),
            None => {}
        }

        if app.primary.sim.is_done() {
            // TODO Deliver some kind of final score
        }

        (None, false)
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_center.draw(g);
    }
}

// TODO Hacks in here, because I'm not convinced programatically specifying this is right. I think
// the Scenario abstractions and UI need to change to make this convenient to express in JSON / the
// UI.

// Motivate a separate left turn phase for north/south, but not left/right
pub fn tutorial_scenario_lvl1(map: &Map) -> ScenarioGenerator {
    // TODO In lieu of the deleted labels
    let north = IntersectionID(2);
    let south = IntersectionID(3);
    // Hush, east/west is more cognitive overhead for me. >_<
    let left = IntersectionID(1);
    let right = IntersectionID(0);

    let mut s = ScenarioGenerator::empty("tutorial lvl1");

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
pub fn tutorial_scenario_lvl2(map: &Map) -> ScenarioGenerator {
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

fn heavy(s: &mut ScenarioGenerator, map: &Map, from: IntersectionID, to: IntersectionID) {
    spawn(s, map, from, to, 100, 0);
}
fn heavy_peds(s: &mut ScenarioGenerator, map: &Map, from: IntersectionID, to: IntersectionID) {
    spawn(s, map, from, to, 0, 100);
}
fn medium(s: &mut ScenarioGenerator, map: &Map, from: IntersectionID, to: IntersectionID) {
    spawn(s, map, from, to, 100, 0);
}
fn light(s: &mut ScenarioGenerator, map: &Map, from: IntersectionID, to: IntersectionID) {
    spawn(s, map, from, to, 100, 0);
}

fn spawn(
    s: &mut ScenarioGenerator,
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
