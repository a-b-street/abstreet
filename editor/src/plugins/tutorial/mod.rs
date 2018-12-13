use crate::objects::Ctx;
use crate::plugins::{Plugin, PluginCtx};
use ezgui::{GfxCtx, LogScroller};

pub enum TutorialMode {
    GiveInstructions(LogScroller),
    Play,
}

impl TutorialMode {
    pub fn new() -> TutorialMode {
        TutorialMode::GiveInstructions(LogScroller::new_from_lines(vec![
            "Welcome to the A/B Street tutorial!".to_string(),
            "".to_string(),
            "There'll be some instructions here eventually. Fix all the traffic!".to_string(),
        ]))
    }
}

impl Plugin for TutorialMode {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        match self {
            TutorialMode::GiveInstructions(ref mut scroller) => {
                if scroller.event(&mut ctx.input) {
                    setup_scenario(ctx);
                    *self = TutorialMode::Play;
                }
                true
            }
            TutorialMode::Play => false,
        }
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        match self {
            TutorialMode::GiveInstructions(ref scroller) => {
                scroller.draw(g, ctx.canvas);
            }
            TutorialMode::Play => {}
        };
    }
}

fn setup_scenario(ctx: &mut PluginCtx) {
    use sim::{BorderSpawnOverTime, OriginDestination, Scenario, Tick};
    let map = &ctx.primary.map;

    fn border_spawn(ctx: &PluginCtx, from: &str, to: &str) -> BorderSpawnOverTime {
        BorderSpawnOverTime {
            // TODO Can we express something like "100 cars per minute, for an hour"
            num_peds: 0,
            num_cars: 100 * 10,
            num_bikes: 0,
            percent_use_transit: 0.0,
            start_tick: Tick::zero(),
            stop_tick: Tick::from_minutes(10),
            start_from_border: ctx.primary.map.intersection(from),
            goal: OriginDestination::Border(ctx.primary.map.intersection(to)),
        }
    }

    Scenario {
        scenario_name: "tutorial_1_left_turns".to_string(),
        map_name: map.get_name().to_string(),
        seed_parked_cars: vec![],
        spawn_over_time: vec![],
        border_spawn_over_time: vec![
            // TODO ideally specify the relative spawning rates here, so some sides can be
            // imbalanced
            border_spawn(ctx, "south", "west"),
            border_spawn(ctx, "north", "south"),
        ],
    }
    .instantiate(&mut ctx.primary.sim, map);
}
