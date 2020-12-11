use std::collections::BTreeSet;

use abstutil::Timer;
use geom::{ArrowCap, Distance, Duration, PolyLine, Pt2D, Time};
use map_gui::tools::{grey_out_map, Minimap, PopupMsg};
use map_gui::ID;
use map_model::raw::OriginalRoad;
use map_model::{osm, BuildingID, Map, Position};
use sim::{
    AgentID, BorderSpawnOverTime, CarID, IndividTrip, PersonSpec, Scenario, ScenarioGenerator,
    SpawnOverTime, TripEndpoint, TripMode, TripPurpose, VehicleType,
};
use widgetry::{
    hotkeys, lctrl, Btn, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    RewriteColor, ScreenPt, State, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::challenges::cutscene::CutsceneBuilder;
use crate::common::{tool_panel, MinimapController, Warping};
use crate::edit::EditMode;
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::{
    maybe_exit_sandbox, spawn_agents_around, Actions, AgentMeter, SandboxControls, SandboxMode,
    SpeedControls, TimePanel,
};

const ESCORT: CarID = CarID(0, VehicleType::Car);
const CAR_BIKE_CONTENTION_GOAL: Duration = Duration::const_seconds(15.0);

pub struct Tutorial {
    top_center: Panel,
    last_finished_task: Task,

    msg_panel: Option<Panel>,
    warped: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TutorialPointer {
    pub stage: usize,
    // Index into messages. messages.len() means the actual task.
    pub part: usize,
}

impl TutorialPointer {
    pub fn new(stage: usize, part: usize) -> TutorialPointer {
        TutorialPointer { stage, part }
    }
}

impl Tutorial {
    /// Launches the tutorial gameplay along with its cutscene
    pub fn start(ctx: &mut EventCtx, app: &mut App) -> Transition {
        Tutorial::initialize(ctx, app);

        Transition::Multi(vec![
            // Constructing the intro_story cutscene doesn't require the map/scenario to be loaded.
            Transition::Push(SandboxMode::simple_new(
                ctx,
                app,
                GameplayMode::Tutorial(
                    app.session
                        .tutorial
                        .as_ref()
                        .map(|tut| tut.current)
                        .unwrap_or(TutorialPointer::new(0, 0)),
                ),
            )),
            Transition::Push(intro_story(ctx, app)),
        ])
    }

    /// Idempotent. This must be called before `make_gameplay` or `scenario`.
    pub fn initialize(ctx: &mut EventCtx, app: &mut App) {
        if app.session.tutorial.is_none() {
            app.session.tutorial = Some(TutorialState::new(ctx, app));
        }
    }

    pub fn make_gameplay(
        ctx: &mut EventCtx,
        app: &mut App,
        current: TutorialPointer,
    ) -> Box<dyn GameplayState> {
        let mut tut = app.session.tutorial.take().unwrap();
        tut.current = current;
        let state = tut.make_state(ctx, app);
        app.session.tutorial = Some(tut);
        state
    }

    pub fn scenario(app: &App, current: TutorialPointer) -> Option<ScenarioGenerator> {
        app.session.tutorial.as_ref().unwrap().stages[current.stage]
            .make_scenario
            .clone()
    }

    fn inner_event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        controls: &mut SandboxControls,
        tut: &mut TutorialState,
    ) -> Option<Transition> {
        // First of all, might need to initiate warping
        if !self.warped {
            if let Some((ref id, zoom)) = tut.stage().warp_to {
                self.warped = true;
                return Some(Transition::Push(Warping::new(
                    ctx,
                    app.primary.canonical_point(id.clone()).unwrap(),
                    Some(zoom),
                    None,
                    &mut app.primary,
                )));
            }
        }

        match self.top_center.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Quit" => {
                    return Some(maybe_exit_sandbox(ctx));
                }
                "previous tutorial" => {
                    tut.current = TutorialPointer::new(tut.current.stage - 1, 0);
                    return Some(transition(ctx, app, tut));
                }
                "next tutorial" => {
                    tut.current = TutorialPointer::new(tut.current.stage + 1, 0);
                    return Some(transition(ctx, app, tut));
                }
                "instructions" => {
                    tut.current = TutorialPointer::new(tut.current.stage, 0);
                    return Some(transition(ctx, app, tut));
                }
                "edit map" => {
                    // TODO Ideally this would be an inactive button in message states
                    if self.msg_panel.is_none() {
                        let mode = GameplayMode::Tutorial(tut.current);
                        return Some(Transition::Push(EditMode::new(ctx, app, mode)));
                    }
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        if let Some(ref mut msg) = self.msg_panel {
            match msg.event(ctx) {
                Outcome::Clicked(x) => match x.as_ref() {
                    "previous message" => {
                        tut.prev();
                        return Some(transition(ctx, app, tut));
                    }
                    "next message" | "Try it" => {
                        tut.next();
                        return Some(transition(ctx, app, tut));
                    }
                    _ => unreachable!(),
                },
                _ => {
                    // Don't allow other interactions
                    return Some(Transition::Keep);
                }
            }
        }

        // Interaction things
        if tut.interaction() == Task::Camera {
            if app.primary.current_selection == Some(ID::Building(tut.fire_station))
                && app.per_obj.left_click(ctx, "put out the... fire?")
            {
                tut.next();
                return Some(transition(ctx, app, tut));
            }
        } else if tut.interaction() == Task::InspectObjects {
            // TODO Have to wiggle the mouse or something after opening the panel, because of the
            // order in SandboxMode.
            match controls.common.as_ref().unwrap().info_panel_open(app) {
                Some(ID::Lane(l)) => {
                    if app.primary.map.get_l(l).is_biking() && !tut.inspected_bike_lane {
                        tut.inspected_bike_lane = true;
                        self.top_center = tut.make_top_center(ctx, false);
                    }
                }
                Some(ID::Building(_)) => {
                    if !tut.inspected_building {
                        tut.inspected_building = true;
                        self.top_center = tut.make_top_center(ctx, false);
                    }
                }
                Some(ID::Intersection(i)) => {
                    let i = app.primary.map.get_i(i);
                    if i.is_stop_sign() && !tut.inspected_stop_sign {
                        tut.inspected_stop_sign = true;
                        self.top_center = tut.make_top_center(ctx, false);
                    }
                    if i.is_border() && !tut.inspected_border {
                        tut.inspected_border = true;
                        self.top_center = tut.make_top_center(ctx, false);
                    }
                }
                _ => {}
            }
            if tut.inspected_bike_lane
                && tut.inspected_building
                && tut.inspected_stop_sign
                && tut.inspected_border
            {
                tut.next();
                return Some(transition(ctx, app, tut));
            }
        } else if tut.interaction() == Task::TimeControls {
            if app.primary.sim.time() >= Time::START_OF_DAY + Duration::hours(17) {
                tut.next();
                return Some(transition(ctx, app, tut));
            }
        } else if tut.interaction() == Task::PauseResume {
            let is_paused = controls.speed.as_ref().unwrap().is_paused();
            if tut.was_paused && !is_paused {
                tut.was_paused = false;
            }
            if !tut.was_paused && is_paused {
                tut.num_pauses += 1;
                tut.was_paused = true;
                self.top_center = tut.make_top_center(ctx, false);
            }
            if tut.num_pauses == 3 {
                tut.next();
                return Some(transition(ctx, app, tut));
            }
        } else if tut.interaction() == Task::Escort {
            let following_car =
                controls.common.as_ref().unwrap().info_panel_open(app) == Some(ID::Car(ESCORT));
            let is_parked = app
                .primary
                .sim
                .agent_to_trip(AgentID::Car(ESCORT))
                .is_none();
            if !tut.car_parked && is_parked && tut.following_car {
                tut.car_parked = true;
                self.top_center = tut.make_top_center(ctx, false);
            }

            if following_car && !tut.following_car {
                // TODO There's a delay of one event before the checklist updates, because the
                // info panel opening happens at the end of the event. Not a big deal.
                tut.following_car = true;
                self.top_center = tut.make_top_center(ctx, false);
            }

            if tut.prank_done {
                tut.next();
                return Some(transition(ctx, app, tut));
            }
        } else if tut.interaction() == Task::LowParking {
            if tut.parking_found {
                tut.next();
                return Some(transition(ctx, app, tut));
            }
        } else if tut.interaction() == Task::WatchBikes {
            if app.primary.sim.time() >= Time::START_OF_DAY + Duration::minutes(3) {
                tut.next();
                return Some(transition(ctx, app, tut));
            }
        } else if tut.interaction() == Task::FixBikes {
            if app.primary.sim.is_done() {
                let mut before = Duration::ZERO;
                let mut after = Duration::ZERO;
                for (_, b, a, _) in app
                    .primary
                    .sim
                    .get_analytics()
                    .both_finished_trips(app.primary.sim.get_end_of_day(), app.prebaked())
                {
                    before = before.max(b);
                    after = after.max(a);
                }
                if !tut.score_delivered {
                    tut.score_delivered = true;
                    if before == after {
                        return Some(Transition::Push(PopupMsg::new(
                            ctx,
                            "All trips completed",
                            vec![
                                "Your changes didn't affect anything!",
                                "Try editing the map to create some bike lanes.",
                            ],
                        )));
                    }
                    if after > before {
                        return Some(Transition::Push(PopupMsg::new(
                            ctx,
                            "All trips completed",
                            vec![
                                "Your changes made things worse!".to_string(),
                                format!(
                                    "All trips originally finished in {}, but now they took {}",
                                    before, after
                                ),
                                "".to_string(),
                                "Try again!".to_string(),
                            ],
                        )));
                    }
                    if before - after < CAR_BIKE_CONTENTION_GOAL {
                        return Some(Transition::Push(PopupMsg::new(
                            ctx,
                            "All trips completed",
                            vec![
                                "Nice, you helped things a bit!".to_string(),
                                format!(
                                    "All trips originally took {}, but now they took {}",
                                    before, after
                                ),
                                "".to_string(),
                                "See if you can do a little better though.".to_string(),
                            ],
                        )));
                    }
                    return Some(Transition::Push(PopupMsg::new(
                        ctx,
                        "All trips completed",
                        vec![format!(
                            "Awesome! All trips originally took {}, but now they only took {}",
                            before, after
                        )],
                    )));
                }
                if before - after >= CAR_BIKE_CONTENTION_GOAL {
                    tut.next();
                }
                return Some(transition(ctx, app, tut));
            }
        } else if tut.interaction() == Task::Done {
            // If the player chooses to stay here, at least go back to the message panel.
            tut.prev();
            return Some(maybe_exit_sandbox(ctx));
        }

        None
    }
}

impl GameplayState for Tutorial {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        controls: &mut SandboxControls,
        _: &mut Actions,
    ) -> Option<Transition> {
        // Dance around borrow-checker issues
        let mut tut = app.session.tutorial.take().unwrap();

        // The arrows get screwy when window size changes.
        let window_dims = (ctx.canvas.window_width, ctx.canvas.window_height);
        if window_dims != tut.window_dims {
            tut.stages = TutorialState::new(ctx, app).stages;
            tut.window_dims = window_dims;
        }

        let result = self.inner_event(ctx, app, controls, &mut tut);
        app.session.tutorial = Some(tut);
        result
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let tut = app.session.tutorial.as_ref().unwrap();

        if self.msg_panel.is_some() {
            grey_out_map(g, app);
        }

        self.top_center.draw(g);

        if let Some(ref msg) = self.msg_panel {
            // Arrows underneath the message panel, but on top of other panels
            if let Some((_, _, Some(fxn))) = tut.lines() {
                let pt = (fxn)(g, app);
                g.fork_screenspace();
                if let Ok(pl) = PolyLine::new(vec![
                    self.msg_panel
                        .as_ref()
                        .unwrap()
                        .center_of("next message")
                        .to_pt(),
                    pt,
                ]) {
                    g.draw_polygon(
                        Color::RED,
                        pl.make_arrow(Distance::meters(20.0), ArrowCap::Triangle),
                    );
                }
                g.unfork();
            }

            msg.draw(g);
        }

        // Special things
        if tut.interaction() == Task::Camera {
            g.draw_polygon(
                Color::hex("#e25822"),
                app.primary.map.get_b(tut.fire_station).polygon.clone(),
            );
        }
    }

    fn recreate_panels(&mut self, ctx: &mut EventCtx, app: &App) {
        let tut = app.session.tutorial.as_ref().unwrap();
        self.top_center = tut.make_top_center(ctx, self.last_finished_task >= Task::WatchBikes);

        // Time can't pass while self.msg_panel is active
    }

    fn can_move_canvas(&self) -> bool {
        self.msg_panel.is_none()
    }
    fn can_examine_objects(&self) -> bool {
        self.last_finished_task >= Task::WatchBikes
    }
    fn has_common(&self) -> bool {
        self.last_finished_task >= Task::Camera
    }
    fn has_tool_panel(&self) -> bool {
        true
    }
    fn has_time_panel(&self) -> bool {
        self.last_finished_task >= Task::InspectObjects
    }
    fn has_speed(&self) -> bool {
        self.last_finished_task >= Task::InspectObjects
    }
    fn has_agent_meter(&self) -> bool {
        self.last_finished_task >= Task::PauseResume
    }
    fn has_minimap(&self) -> bool {
        self.last_finished_task >= Task::Escort
    }
}

#[derive(PartialEq, PartialOrd, Clone, Copy)]
enum Task {
    Nil,
    Camera,
    InspectObjects,
    TimeControls,
    PauseResume,
    Escort,
    LowParking,
    WatchBikes,
    FixBikes,
    Done,
}

impl Task {
    fn top_txt(self, state: &TutorialState) -> Text {
        let simple = match self {
            Task::Nil => unreachable!(),
            Task::Camera => "Put out the fire at the fire station",
            Task::InspectObjects => {
                let mut txt = Text::from(Line("Find one of each:"));
                for (name, done) in vec![
                    ("bike lane", state.inspected_bike_lane),
                    ("building", state.inspected_building),
                    ("intersection with stop sign", state.inspected_stop_sign),
                    ("intersection on the map border", state.inspected_border),
                ] {
                    if done {
                        txt.add(Line(format!("[X] {}", name)).fg(Color::GREEN));
                    } else {
                        txt.add(Line(format!("[ ] {}", name)));
                    }
                }
                return txt;
            }
            Task::TimeControls => "Wait until after 5pm",
            Task::PauseResume => {
                let mut txt = Text::from(Line("[ ] Pause/resume "));
                txt.append(Line(format!("{} times", 3 - state.num_pauses)).fg(Color::GREEN));
                return txt;
            }
            Task::Escort => {
                // Inspect the target car, wait for them to park, draw WASH ME on the window
                let mut txt = Text::new();
                if state.following_car {
                    txt.add(Line("[X] follow the target car").fg(Color::GREEN));
                } else {
                    txt.add(Line("[ ] follow the target car"));
                }
                if state.car_parked {
                    txt.add(Line("[X] wait for them to park").fg(Color::GREEN));
                } else {
                    txt.add(Line("[ ] wait for them to park"));
                }
                if state.prank_done {
                    txt.add(Line("[X] click car and press c to draw WASH ME").fg(Color::GREEN));
                } else {
                    txt.add(Line("[ ] click car and press "));
                    // TODO ctx.style().hotkey_color
                    txt.append(Line(Key::C.describe()).fg(Color::GREEN));
                    txt.append(Line(" to draw WASH ME"));
                }
                return txt;
            }
            Task::LowParking => {
                let mut txt = Text::from(Line(
                    "1) Find a road with almost no parking spots available",
                ));
                txt.add(Line("2) Click it and press "));
                // TODO ctx.style().hotkey_color
                txt.append(Line(Key::C.describe()).fg(Color::GREEN));
                txt.append(Line(" to check the occupancy"));
                return txt;
            }
            Task::WatchBikes => "Watch for 3 minutes",
            Task::FixBikes => {
                return Text::from(Line(format!(
                    "[ ] Complete all trips {} faster",
                    CAR_BIKE_CONTENTION_GOAL
                )));
            }
            Task::Done => "Tutorial complete!",
        };
        Text::from(Line(simple))
    }

    fn label(self) -> &'static str {
        match self {
            Task::Nil => unreachable!(),
            Task::Camera => "Moving the drone",
            Task::InspectObjects => "Interacting with objects",
            Task::TimeControls => "Passing the time",
            Task::PauseResume => "Pausing/resuming",
            Task::Escort => "Following people",
            Task::LowParking => "Exploring map layers",
            Task::WatchBikes => "Observing a problem",
            Task::FixBikes => "Editing lanes",
            Task::Done => "Tutorial complete!",
        }
    }
}

struct Stage {
    messages: Vec<(
        Vec<String>,
        HorizontalAlignment,
        Option<Box<dyn Fn(&GfxCtx, &App) -> Pt2D>>,
    )>,
    task: Task,
    warp_to: Option<(ID, f64)>,
    custom_spawn: Option<Box<dyn Fn(&mut App)>>,
    make_scenario: Option<ScenarioGenerator>,
}

fn arrow(pt: ScreenPt) -> Option<Box<dyn Fn(&GfxCtx, &App) -> Pt2D>> {
    Some(Box::new(move |_, _| pt.to_pt()))
}

impl Stage {
    fn new(task: Task) -> Stage {
        Stage {
            messages: Vec::new(),
            task,
            warp_to: None,
            custom_spawn: None,
            make_scenario: None,
        }
    }

    fn msg<I: Into<String>>(
        mut self,
        lines: Vec<I>,
        point_to: Option<Box<dyn Fn(&GfxCtx, &App) -> Pt2D>>,
    ) -> Stage {
        self.messages.push((
            lines.into_iter().map(|l| l.into()).collect(),
            HorizontalAlignment::Center,
            point_to,
        ));
        self
    }
    fn left_aligned_msg<I: Into<String>>(
        mut self,
        lines: Vec<I>,
        point_to: Option<Box<dyn Fn(&GfxCtx, &App) -> Pt2D>>,
    ) -> Stage {
        self.messages.push((
            lines.into_iter().map(|l| l.into()).collect(),
            HorizontalAlignment::Left,
            point_to,
        ));
        self
    }

    fn warp_to(mut self, id: ID, zoom: Option<f64>) -> Stage {
        assert!(self.warp_to.is_none());
        self.warp_to = Some((id, zoom.unwrap_or(4.0)));
        self
    }

    fn custom_spawn(mut self, cb: Box<dyn Fn(&mut App)>) -> Stage {
        assert!(self.custom_spawn.is_none());
        self.custom_spawn = Some(cb);
        self
    }

    fn scenario(mut self, generator: ScenarioGenerator) -> Stage {
        assert!(self.make_scenario.is_none());
        self.make_scenario = Some(generator);
        self
    }
}

pub struct TutorialState {
    stages: Vec<Stage>,
    pub current: TutorialPointer,

    window_dims: (f64, f64),

    // Goofy state for just some stages.
    inspected_bike_lane: bool,
    inspected_building: bool,
    inspected_stop_sign: bool,
    inspected_border: bool,

    was_paused: bool,
    num_pauses: usize,

    following_car: bool,
    car_parked: bool,
    prank_done: bool,

    parking_found: bool,

    score_delivered: bool,

    fire_station: BuildingID,
}

fn make_bike_lane_scenario(map: &Map) -> ScenarioGenerator {
    let mut s = ScenarioGenerator::empty("car vs bike contention");
    s.border_spawn_over_time.push(BorderSpawnOverTime {
        num_peds: 0,
        num_cars: 10,
        num_bikes: 10,
        percent_use_transit: 0.0,
        start_time: Time::START_OF_DAY,
        stop_time: Time::START_OF_DAY + Duration::seconds(10.0),
        start_from_border: map.find_i_by_osm_id(osm::NodeID(3005680098)).unwrap(),
        goal: Some(TripEndpoint::Bldg(
            map.find_b_by_osm_id(bldg(217699501)).unwrap(),
        )),
    });
    s
}

fn transition(ctx: &mut EventCtx, app: &mut App, tut: &mut TutorialState) -> Transition {
    tut.reset_state();
    let mode = GameplayMode::Tutorial(tut.current);
    Transition::Replace(SandboxMode::simple_new(ctx, app, mode))
}

impl TutorialState {
    // These're mutex to each state, but still important to reset. Otherwise if you go back to a
    // previous interaction stage, it'll just be automatically marked done.
    fn reset_state(&mut self) {
        self.inspected_bike_lane = false;
        self.inspected_building = false;
        self.inspected_stop_sign = false;
        self.inspected_border = false;
        self.was_paused = true;
        self.num_pauses = 0;
        self.score_delivered = false;
        self.following_car = false;
        self.car_parked = false;
        self.prank_done = false;
        self.parking_found = false;
    }

    fn stage(&self) -> &Stage {
        &self.stages[self.current.stage]
    }

    fn interaction(&self) -> Task {
        let stage = self.stage();
        if self.current.part == stage.messages.len() {
            stage.task
        } else {
            Task::Nil
        }
    }
    fn lines(
        &self,
    ) -> Option<&(
        Vec<String>,
        HorizontalAlignment,
        Option<Box<dyn Fn(&GfxCtx, &App) -> Pt2D>>,
    )> {
        let stage = self.stage();
        if self.current.part == stage.messages.len() {
            None
        } else {
            Some(&stage.messages[self.current.part])
        }
    }

    fn next(&mut self) {
        self.current.part += 1;
        if self.current.part == self.stage().messages.len() + 1 {
            self.current = TutorialPointer::new(self.current.stage + 1, 0);
        }
    }
    fn prev(&mut self) {
        if self.current.part == 0 {
            self.current = TutorialPointer::new(
                self.current.stage - 1,
                self.stages[self.current.stage - 1].messages.len(),
            );
        } else {
            self.current.part -= 1;
        }
    }

    fn make_top_center(&self, ctx: &mut EventCtx, edit_map: bool) -> Panel {
        let mut col = vec![Widget::row(vec![
            Line("Tutorial").small_heading().draw(ctx),
            Widget::vert_separator(ctx, 50.0),
            if self.current.stage == 0 {
                Btn::text_fg("<").inactive(ctx)
            } else {
                Btn::text_fg("<").build(ctx, "previous tutorial", None)
            },
            {
                let mut txt = Text::from(Line(format!("Task {}", self.current.stage + 1)));
                // TODO Smaller font and use alpha for the "/9" part
                txt.append(Line(format!("/{}", self.stages.len())).fg(Color::grey(0.7)));
                txt.draw(ctx)
            },
            if self.current.stage == self.stages.len() - 1 {
                Btn::text_fg(">").inactive(ctx)
            } else {
                Btn::text_fg(">").build(ctx, "next tutorial", None)
            },
            Btn::text_fg("Quit").build_def(ctx, None),
        ])
        .centered()];
        {
            let task = self.interaction();
            if task != Task::Nil {
                col.push(Widget::row(vec![
                    Text::from(
                        Line(format!(
                            "Task {}: {}",
                            self.current.stage + 1,
                            self.stage().task.label()
                        ))
                        .small_heading(),
                    )
                    .draw(ctx),
                    // TODO also text saying "instructions"... can we layout two things easily to
                    // make a button?
                    Btn::svg_def("system/assets/tools/info.svg")
                        .build(ctx, "instructions", None)
                        .centered_vert()
                        .align_right(),
                ]));
                col.push(task.top_txt(self).draw(ctx));
            }
        }
        if edit_map {
            col.push(Btn::svg_def("system/assets/tools/edit_map.svg").build(
                ctx,
                "edit map",
                lctrl(Key::E),
            ));
        }

        Panel::new(Widget::col(col))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx)
    }

    fn make_state(&self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn GameplayState> {
        if self.interaction() == Task::Nil {
            app.primary.current_selection = None;
        }

        if let Some(ref cb) = self.stage().custom_spawn {
            (cb)(app);
            app.primary
                .sim
                .tiny_step(&app.primary.map, &mut app.primary.sim_cb);
        }
        // If this stage has a scenario, it's instantiated when SandboxMode gets created.

        let last_finished_task = if self.current.stage == 0 {
            Task::Nil
        } else {
            self.stages[self.current.stage - 1].task
        };

        Box::new(Tutorial {
            top_center: self.make_top_center(ctx, last_finished_task >= Task::WatchBikes),
            last_finished_task,

            msg_panel: if let Some((ref lines, horiz_align, _)) = self.lines() {
                let mut col = vec![{
                    let mut txt = Text::new();
                    txt.add(Line(self.stage().task.label()).small_heading());
                    txt.add(Line(""));

                    for l in lines {
                        txt.add(Line(l));
                    }
                    txt.wrap_to_pct(ctx, 30).draw(ctx)
                }];
                let mut controls = vec![Widget::row(vec![
                    if self.current.part > 0 {
                        Btn::svg(
                            "system/assets/tools/prev.svg",
                            RewriteColor::Change(Color::WHITE, app.cs.hovering),
                        )
                        .build(ctx, "previous message", Key::LeftArrow)
                    } else {
                        Widget::draw_svg_transform(
                            ctx,
                            "system/assets/tools/prev.svg",
                            RewriteColor::ChangeAll(Color::WHITE.alpha(0.5)),
                        )
                    },
                    format!("{}/{}", self.current.part + 1, self.stage().messages.len())
                        .draw_text(ctx)
                        .centered_vert(),
                    if self.current.part == self.stage().messages.len() - 1 {
                        Widget::draw_svg_transform(
                            ctx,
                            "system/assets/tools/next.svg",
                            RewriteColor::ChangeAll(Color::WHITE.alpha(0.5)),
                        )
                        .named("next message")
                    } else {
                        Btn::svg(
                            "system/assets/tools/next.svg",
                            RewriteColor::Change(Color::WHITE, app.cs.hovering),
                        )
                        .build(
                            ctx,
                            "next message",
                            hotkeys(vec![Key::RightArrow, Key::Space, Key::Enter]),
                        )
                    },
                ])];
                if self.current.part == self.stage().messages.len() - 1 {
                    controls.push(
                        Btn::text_bg2("Try it")
                            .build_def(ctx, hotkeys(vec![Key::RightArrow, Key::Space, Key::Enter])),
                    );
                }
                col.push(Widget::col(controls).align_bottom());

                Some(
                    Panel::new(Widget::col(col).outline(5.0, Color::WHITE))
                        .exact_size_percent(40, 40)
                        .aligned(*horiz_align, VerticalAlignment::Center)
                        .build(ctx),
                )
            } else {
                None
            },
            warped: false,
        })
    }

    fn new(ctx: &mut EventCtx, app: &App) -> TutorialState {
        let mut state = TutorialState {
            stages: Vec::new(),
            current: TutorialPointer::new(0, 0),
            window_dims: (ctx.canvas.window_width, ctx.canvas.window_height),

            inspected_bike_lane: false,
            inspected_building: false,
            inspected_stop_sign: false,
            inspected_border: false,
            was_paused: true,
            num_pauses: 0,
            following_car: false,
            car_parked: false,
            prank_done: false,
            parking_found: false,
            score_delivered: false,

            fire_station: app.primary.map.find_b_by_osm_id(bldg(731238736)).unwrap(),
        };

        let tool_panel = tool_panel(ctx);
        let time = TimePanel::new(ctx, app);
        let speed = SpeedControls::new(ctx, app);
        let agent_meter = AgentMeter::new(ctx, app);
        // The minimap is hidden at low zoom levels
        let orig_zoom = ctx.canvas.cam_zoom;
        ctx.canvas.cam_zoom = 100.0;
        let minimap = Minimap::new(ctx, app, MinimapController);
        ctx.canvas.cam_zoom = orig_zoom;

        let map = &app.primary.map;

        state.stages.push(
            Stage::new(Task::Camera)
                .warp_to(
                    ID::Intersection(map.find_i_by_osm_id(osm::NodeID(53096945)).unwrap()),
                    None,
                )
                .msg(
                    vec![
                        "Let's start by piloting your fancy new drone.",
                        "",
                        "- Click and drag to pan around the map",
                        "- Use your scroll wheel or touchpad to zoom in and out.",
                    ],
                    None,
                )
                .msg(
                    vec!["If the controls feel wrong, try adjusting the settings."],
                    arrow(tool_panel.center_of("settings")),
                )
                .msg(
                    vec![
                        "Let's try the drone ou--",
                        "",
                        "WHOA, THERE'S A FIRE STATION ON FIRE!",
                        "GO CLICK ON IT, QUICK!",
                    ],
                    None,
                )
                .msg(
                    vec![
                        "Hint:",
                        "- Look around for an unusually red building",
                        "- You have to zoom in to interact with anything on the map.",
                    ],
                    None,
                ),
        );

        state.stages.push(
            Stage::new(Task::InspectObjects)
                .msg(
                    vec![
                        "What, no fire? Er, sorry about that. Just a little joke we like to play \
                         on the new recruits.",
                    ],
                    None,
                )
                .msg(
                    vec![
                        "Now, let's learn how to inspect and interact with objects in the map.",
                        "",
                        "- Click on something.",
                        "- Hint: You have to zoom in before you can select anything.",
                    ],
                    None,
                ),
        );

        state.stages.push(
            Stage::new(Task::TimeControls)
                .warp_to(
                    ID::Intersection(map.find_i_by_osm_id(osm::NodeID(53096945)).unwrap()),
                    Some(6.5),
                )
                .msg(
                    vec![
                        "Inspection complete!",
                        "",
                        "You'll work day and night, watching traffic patterns unfold.",
                    ],
                    arrow(time.panel.center_of_panel()),
                )
                .msg(
                    vec!["You can pause or resume time"],
                    arrow(speed.panel.center_of("pause")),
                )
                .msg(
                    vec![
                        "Speed things up",
                        "",
                        "(The keyboard shortcuts are very helpful here!)",
                    ],
                    arrow(speed.panel.center_of("30x speed")),
                )
                .msg(
                    vec!["Advance time by certain amounts"],
                    arrow(speed.panel.center_of("step forwards")),
                )
                .msg(
                    vec!["And jump to the beginning of the day"],
                    arrow(speed.panel.center_of("reset to midnight")),
                )
                .msg(
                    vec!["Let's try these controls out. Wait until 5pm or later."],
                    None,
                ),
        );

        state.stages.push(
            Stage::new(Task::PauseResume)
                .msg(
                    vec!["Whew, that took a while! (Hopefully not though...)"],
                    None,
                )
                .msg(
                    vec![
                        "You might've figured it out already,",
                        "But you'll be pausing/resuming time VERY frequently",
                    ],
                    arrow(speed.panel.center_of("pause")),
                )
                .msg(
                    vec!["Just reassure me and pause/resume time a few times, alright?"],
                    None,
                ),
        );

        state.stages.push(
            Stage::new(Task::Escort)
                // Don't center on where the agents are, be a little offset
                .warp_to(
                    ID::Building(map.find_b_by_osm_id(bldg(217699780)).unwrap()),
                    Some(10.0),
                )
                .custom_spawn(Box::new(move |app| {
                    // Seed a specific target car, and fill up the target building's private
                    // parking to force the target to park on-street.
                    let map = &app.primary.map;
                    let goal_bldg = map.find_b_by_osm_id(bldg(217701875)).unwrap();
                    let start_lane = {
                        let r = map.get_r(
                            map.find_r_by_osm_id(OriginalRoad::new(
                                158782224,
                                (1709145066, 53128052),
                            ))
                            .unwrap(),
                        );
                        assert_eq!(r.lanes_ltr().len(), 6);
                        r.lanes_ltr()[2].0
                    };
                    let lane_near_bldg = {
                        let r = map.get_r(
                            map.find_r_by_osm_id(OriginalRoad::new(6484869, (53163501, 53069236)))
                                .unwrap(),
                        );
                        assert_eq!(r.lanes_ltr().len(), 6);
                        r.lanes_ltr()[3].0
                    };

                    let mut scenario = Scenario::empty(map, "prank");
                    scenario.people.push(PersonSpec {
                        orig_id: None,
                        origin: TripEndpoint::SuddenlyAppear(Position::new(
                            start_lane,
                            map.get_l(start_lane).length() * 0.8,
                        )),
                        trips: vec![IndividTrip::new(
                            Time::START_OF_DAY,
                            TripPurpose::Shopping,
                            TripEndpoint::Bldg(goal_bldg),
                            TripMode::Drive,
                        )],
                    });
                    // Will definitely get there first
                    for _ in 0..map.get_b(goal_bldg).num_parking_spots() {
                        scenario.people.push(PersonSpec {
                            orig_id: None,
                            origin: TripEndpoint::SuddenlyAppear(Position::new(
                                lane_near_bldg,
                                map.get_l(lane_near_bldg).length() / 2.0,
                            )),
                            trips: vec![IndividTrip::new(
                                Time::START_OF_DAY,
                                TripPurpose::Shopping,
                                TripEndpoint::Bldg(goal_bldg),
                                TripMode::Drive,
                            )],
                        });
                    }
                    let mut rng = app.primary.current_flags.sim_flags.make_rng();
                    scenario.instantiate(
                        &mut app.primary.sim,
                        map,
                        &mut rng,
                        &mut Timer::new("spawn trip"),
                    );
                    app.primary.sim.tiny_step(map, &mut app.primary.sim_cb);

                    // And add some noise
                    spawn_agents_around(
                        app.primary
                            .map
                            .find_i_by_osm_id(osm::NodeID(1709145066))
                            .unwrap(),
                        app,
                    );
                }))
                .msg(
                    vec!["Alright alright, no need to wear out your spacebar."],
                    None,
                )
                .msg(
                    vec![
                        "Oh look, some people appeared!",
                        "We've got pedestrians, bikes, and cars moving around now.",
                    ],
                    None,
                )
                .msg(
                    vec!["You can see the number of them here."],
                    arrow(agent_meter.panel.center_of_panel()),
                )
                .left_aligned_msg(
                    vec![
                        "Why don't you follow this car to their destination,",
                        "see where they park, and then play a little... prank?",
                    ],
                    Some(Box::new(|g, app| {
                        g.canvas
                            .map_to_screen(
                                app.primary
                                    .sim
                                    .canonical_pt_for_agent(AgentID::Car(ESCORT), &app.primary.map)
                                    .unwrap(),
                            )
                            .to_pt()
                    })),
                )
                .msg(
                    vec![
                        "You don't have to manually chase them; just click to follow.",
                        "",
                        "(If you do lose track of them, just reset)",
                    ],
                    arrow(speed.panel.center_of("reset to midnight")),
                ),
        );

        state.stages.push(
            Stage::new(Task::LowParking)
                // TODO Actually, we ideally just want a bunch of parked cars, not all these trips
                .scenario(ScenarioGenerator {
                    scenario_name: "low parking".to_string(),
                    only_seed_buses: Some(BTreeSet::new()),
                    spawn_over_time: vec![SpawnOverTime {
                        num_agents: 1000,
                        start_time: Time::START_OF_DAY,
                        stop_time: Time::START_OF_DAY + Duration::hours(3),
                        goal: None,
                        percent_driving: 1.0,
                        percent_biking: 0.0,
                        percent_use_transit: 0.0,
                    }],
                    border_spawn_over_time: Vec::new(),
                })
                .msg(
                    vec![
                        "What an immature prank. You should re-evaluate your life decisions.",
                        "",
                        "The map is quite large, so to help you orient, the minimap shows you an \
                         overview of all activity. You can click and drag it just like the normal \
                         map.",
                    ],
                    arrow(minimap.get_panel().center_of("minimap")),
                )
                .msg(
                    vec![
                        "You can apply different layers to the map, to find things like:",
                        "",
                        "- roads with high traffic",
                        "- bus stops",
                        "- how much parking is filled up",
                    ],
                    arrow(minimap.get_panel().center_of("change layers")),
                )
                .msg(
                    vec![
                        "Let's try these out.",
                        "There are lots of cars parked everywhere. Can you find a road that's \
                         almost out of parking spots?",
                    ],
                    None,
                ),
        );

        let bike_lane_scenario = make_bike_lane_scenario(map);
        let bike_lane_focus_pt = map.find_b_by_osm_id(bldg(217699496)).unwrap();

        state.stages.push(
            Stage::new(Task::WatchBikes)
                .warp_to(ID::Building(bike_lane_focus_pt), None)
                .scenario(bike_lane_scenario.clone())
                .msg(
                    vec![
                        "Well done!",
                        "",
                        "Something's about to happen over here. Follow along and figure out what \
                         the problem is, at whatever speed you'd like.",
                    ],
                    None,
                ),
        );

        let top_center = state.make_top_center(ctx, true);
        state.stages.push(
            Stage::new(Task::FixBikes)
                .scenario(bike_lane_scenario)
                .warp_to(ID::Building(bike_lane_focus_pt), None)
                .msg(
                    vec![
                        "Looks like lots of cars and bikes trying to go to a house by the \
                         playfield.",
                        "",
                        "When lots of cars and bikes share the same lane, cars are delayed \
                         (assuming there's no room to pass) and the cyclist probably feels unsafe \
                         too.",
                    ],
                    None,
                )
                .msg(
                    vec![
                        "Luckily, you have the power to modify lanes! What if you could transform \
                         the parking lanes that aren't being used much into bike lanes?",
                    ],
                    None,
                )
                .msg(
                    vec!["To edit lanes, click 'edit map' and then select a lane."],
                    arrow(top_center.center_of("edit map")),
                )
                .msg(
                    vec![
                        "When you finish making edits, time will jump to the beginning of the \
                         next day. You can't make most changes in the middle of the day.",
                        "",
                        "Seattleites are really boring; they follow the exact same schedule \
                         everyday. They're also stubborn, so even if you try to influence their \
                         decision whether to drive, walk, bike, or take a bus, they'll do the \
                         same thing. For now, you're just trying to make things better, assuming \
                         people stick to their routine.",
                    ],
                    None,
                )
                .msg(
                    // TODO Deliberately vague with the measurement.
                    vec![
                        format!(
                            "So adjust lanes and speed up the slowest trip by at least {}.",
                            CAR_BIKE_CONTENTION_GOAL
                        ),
                        "".to_string(),
                        "You can explore results as trips finish. When everyone's finished, \
                         you'll get your final score."
                            .to_string(),
                    ],
                    arrow(agent_meter.panel.center_of("more data")),
                ),
        );

        state.stages.push(Stage::new(Task::Done).msg(
            vec![
                "You're ready for the hard stuff now.",
                "",
                "- Try out some challenges",
                "- Explore larger parts of Seattle in the sandbox, and try out any ideas you've \
                 got.",
                "- Check out community proposals, and submit your own",
                "",
                "Go have the appropriate amount of fun!",
            ],
            None,
        ));

        state

        // TODO Multi-modal trips -- including parking. (Cars per bldg, ownership)
        // TODO Explain the finished trip data
        // The city is in total crisis. You've only got 10 days to do something before all hell
        // breaks loose and people start kayaking / ziplining / crab-walking / cartwheeling / to
        // work.
    }

    pub fn scenarios_to_prebake(map: &Map) -> Vec<ScenarioGenerator> {
        vec![make_bike_lane_scenario(map)]
    }
}

pub fn actions(app: &App, id: ID) -> Vec<(Key, String)> {
    match (app.session.tutorial.as_ref().unwrap().interaction(), id) {
        (Task::LowParking, ID::Lane(_)) => {
            vec![(Key::C, "check the parking occupancy".to_string())]
        }
        (Task::Escort, ID::Car(_)) => vec![(Key::C, "draw WASH ME".to_string())],
        _ => Vec::new(),
    }
}

pub fn execute(ctx: &mut EventCtx, app: &mut App, id: ID, action: &str) -> Transition {
    let mut tut = app.session.tutorial.as_mut().unwrap();
    let response = match (id, action.as_ref()) {
        (ID::Car(c), "draw WASH ME") => {
            let is_parked = app
                .primary
                .sim
                .agent_to_trip(AgentID::Car(ESCORT))
                .is_none();
            if c == ESCORT {
                if is_parked {
                    tut.prank_done = true;
                    PopupMsg::new(
                        ctx,
                        "Prank in progress",
                        vec!["You quickly scribble on the window..."],
                    )
                } else {
                    PopupMsg::new(
                        ctx,
                        "Not yet!",
                        vec![
                            "You're going to run up to an occupied car and draw on their windows?",
                            "Sounds like we should be friends.",
                            "But, er, wait for the car to park. (You can speed up time!)",
                        ],
                    )
                }
            } else if c.1 == VehicleType::Bike {
                PopupMsg::new(
                    ctx,
                    "That's a bike",
                    vec![
                        "Achievement unlocked: You attempted to draw WASH ME on a cyclist.",
                        "This game is PG-13 or something, so I can't really describe what happens \
                         next.",
                        "But uh, don't try this at home.",
                    ],
                )
            } else {
                PopupMsg::new(
                    ctx,
                    "Wrong car",
                    vec![
                        "You're looking at the wrong car.",
                        "Use the 'reset to midnight' (key binding 'X') to start over, if you lost \
                         the car to follow.",
                    ],
                )
            }
        }
        (ID::Lane(l), "check the parking occupancy") => {
            let lane = app.primary.map.get_l(l);
            if lane.is_parking() {
                let percent = (app.primary.sim.get_free_onstreet_spots(l).len() as f64)
                    / (lane.number_parking_spots() as f64);
                if percent > 0.1 {
                    PopupMsg::new(
                        ctx,
                        "Not quite",
                        vec![
                            format!("This lane has {:.0}% spots free", percent * 100.0),
                            "Try using the 'parking occupancy' layer from the minimap controls"
                                .to_string(),
                        ],
                    )
                } else {
                    tut.parking_found = true;
                    PopupMsg::new(
                        ctx,
                        "Noice",
                        vec!["Yup, parallel parking would be tough here!"],
                    )
                }
            } else {
                PopupMsg::new(ctx, "Uhh..", vec!["That's not even a parking lane"])
            }
        }
        _ => unreachable!(),
    };
    Transition::Push(response)
}

fn intro_story(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
    CutsceneBuilder::new("Introduction")
        .boss(
            "Argh, the mayor's on my case again about the West Seattle bridge. This day couldn't \
             get any worse.",
        )
        .player("Er, hello? Boss? I'm --")
        .boss("Yet somehow it did.. You're the new recruit. Yeah, yeah. Come in.")
        .boss(
            "Due to budget cuts, we couldn't hire a real traffic engineer, so we just called some \
             know-it-all from Reddit who seems to think they can fix Seattle traffic.",
        )
        .player("Yes, hi, my name is --")
        .boss("We can't afford name-tags, didn't you hear, budget cuts? Your name doesn't matter.")
        .player("What about my Insta handle?")
        .boss("-glare-")
        .boss(
            "Look, you think fixing traffic is easy? Hah! You can't fix one intersection without \
             breaking ten more.",
        )
        .boss(
            "And everybody wants something different! Bike lanes here! More parking! Faster \
             buses! Cheaper housing! Less rain! Free this, subsidized that!",
        )
        .boss("Light rail and robot cars aren't here to save the day! Know what you'll be using?")
        .extra("drone", 1.0, "The traffic drone")
        .player("Is that... duct tape?")
        .boss(
            "Can't spit anymore cause of COVID and don't get me started on prayers. Well, off to \
             training for you!",
        )
        .build(
            ctx,
            app,
            Box::new(|ctx| {
                Text::from(Line("Use the tutorial to learn the basic controls.").fg(Color::BLACK))
                    .draw(ctx)
            }),
        )
}

// Assumes ways
fn bldg(id: i64) -> osm::OsmID {
    osm::OsmID::Way(osm::WayID(id))
}
