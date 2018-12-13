use crate::colors::ColorScheme;
use crate::objects::{Ctx, RenderingHints, ID};
use crate::plugins::{Plugin, PluginCtx};
use crate::render::Renderable;
use crate::state::{DefaultUIState, UIState};
use crate::ui::PerMapUI;
use ezgui::{Canvas, Color, GfxCtx, LogScroller, UserInput};
use sim::SimFlags;

pub struct TutorialState {
    state: DefaultUIState,
}

impl TutorialState {
    pub fn new(flags: SimFlags, canvas: &Canvas) -> TutorialState {
        TutorialState {
            state: DefaultUIState::new(flags, None, canvas),
        }
    }
}

impl UIState for TutorialState {
    fn handle_zoom(&mut self, old_zoom: f64, new_zoom: f64) {
        self.state.handle_zoom(old_zoom, new_zoom);
    }
    fn set_current_selection(&mut self, obj: Option<ID>) {
        self.state.set_current_selection(obj);
    }
    fn event(
        &mut self,
        input: &mut UserInput,
        hints: &mut RenderingHints,
        recalculate_current_selection: &mut bool,
        cs: &mut ColorScheme,
        canvas: &mut Canvas,
    ) {
        self.state
            .event(input, hints, recalculate_current_selection, cs, canvas);
    }
    fn get_objects_onscreen(
        &self,
        canvas: &Canvas,
    ) -> (Vec<Box<&Renderable>>, Vec<Box<Renderable>>) {
        self.state.get_objects_onscreen(canvas)
    }
    fn is_debug_mode_enabled(&self) -> bool {
        self.state.is_debug_mode_enabled()
    }
    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        self.state.draw(g, ctx);
    }
    fn dump_before_abort(&self) {
        self.state.dump_before_abort();
    }
    fn color_obj(&self, id: ID, ctx: &Ctx) -> Option<Color> {
        self.state.color_obj(id, ctx)
    }
    fn primary(&self) -> &PerMapUI {
        self.state.primary()
    }
}

// TODO Bring this stuff back
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
