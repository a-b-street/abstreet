use crate::colors::ColorScheme;
use crate::objects::{Ctx, RenderingHints, ID};
use crate::render::Renderable;
use crate::state::{DefaultUIState, PerMapUI, UIState};
use ezgui::{Canvas, Color, GfxCtx, LogScroller, UserInput};
use sim::SimFlags;

pub struct TutorialState {
    main: DefaultUIState,
    state: State,
}

enum State {
    GiveInstructions(LogScroller),
    Play,
}

impl TutorialState {
    pub fn new(flags: SimFlags, canvas: &Canvas) -> TutorialState {
        TutorialState {
            main: DefaultUIState::new(flags, None, canvas),
            state: State::GiveInstructions(LogScroller::new_from_lines(vec![
                "Welcome to the A/B Street tutorial!".to_string(),
                "".to_string(),
                "There'll be some instructions here eventually. Fix all the traffic!".to_string(),
            ])),
        }
    }
}

impl UIState for TutorialState {
    fn handle_zoom(&mut self, old_zoom: f64, new_zoom: f64) {
        self.main.handle_zoom(old_zoom, new_zoom);
    }
    fn set_current_selection(&mut self, obj: Option<ID>) {
        self.main.set_current_selection(obj);
    }
    fn event(
        &mut self,
        input: &mut UserInput,
        hints: &mut RenderingHints,
        recalculate_current_selection: &mut bool,
        cs: &mut ColorScheme,
        canvas: &mut Canvas,
    ) {
        match self.state {
            State::GiveInstructions(ref mut scroller) => {
                if scroller.event(input) {
                    setup_scenario(&mut self.main.primary);
                    self.state = State::Play;
                }
            }
            State::Play => {
                self.main
                    .event(input, hints, recalculate_current_selection, cs, canvas);
            }
        }
    }
    fn get_objects_onscreen(
        &self,
        canvas: &Canvas,
    ) -> (Vec<Box<&Renderable>>, Vec<Box<Renderable>>) {
        self.main.get_objects_onscreen(canvas)
    }
    fn is_debug_mode_enabled(&self) -> bool {
        self.main.is_debug_mode_enabled()
    }
    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        match self.state {
            State::GiveInstructions(ref scroller) => {
                scroller.draw(g, ctx.canvas);
            }
            State::Play => {
                self.main.draw(g, ctx);
            }
        }
    }
    fn dump_before_abort(&self) {
        self.main.dump_before_abort();
    }
    fn color_obj(&self, id: ID, ctx: &Ctx) -> Option<Color> {
        self.main.color_obj(id, ctx)
    }
    fn primary(&self) -> &PerMapUI {
        self.main.primary()
    }
}

fn setup_scenario(primary: &mut PerMapUI) {
    use sim::{BorderSpawnOverTime, OriginDestination, Scenario, Tick};
    let map = &primary.map;

    fn border_spawn(primary: &PerMapUI, from: &str, to: &str) -> BorderSpawnOverTime {
        BorderSpawnOverTime {
            // TODO Can we express something like "100 cars per minute, for an hour"
            num_peds: 0,
            num_cars: 100 * 10,
            num_bikes: 0,
            percent_use_transit: 0.0,
            start_tick: Tick::zero(),
            stop_tick: Tick::from_minutes(10),
            start_from_border: primary.map.intersection(from),
            goal: OriginDestination::Border(primary.map.intersection(to)),
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
            border_spawn(primary, "south", "west"),
            border_spawn(primary, "north", "south"),
        ],
    }
    .instantiate(&mut primary.sim, map);
}
