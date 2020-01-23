use crate::common::{CommonState, Minimap, Overlays, Warping};
use crate::game::{msg, State, Transition};
use crate::helpers::ID;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::render::DrawOptions;
use crate::sandbox::{spawn_agents_around, AgentMeter, SpeedControls, TimePanel};
use crate::ui::{ShowEverything, UI};
use abstutil::Timer;
use ezgui::{
    hotkey, Color, Composite, EventCtx, EventLoopMode, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, ManagedWidget, Outcome, ScreenPt, Text, VerticalAlignment,
};
use geom::{Distance, Duration, PolyLine, Polygon, Pt2D, Time};
use map_model::{BuildingID, IntersectionID};
use sim::{AgentID, CarID, Scenario, VehicleType};

pub struct TutorialMode {
    state: TutorialState,

    top_center: Composite,

    msg_panel: Option<Composite>,
    common: Option<CommonState>,
    time_panel: Option<TimePanel>,
    speed: Option<SpeedControls>,
    agent_meter: Option<AgentMeter>,
    minimap: Option<Minimap>,

    // Goofy state for just some stages.
    inspected_lane: bool,
    inspected_building: bool,
    inspected_intersection: bool,
    was_paused: bool,
    num_pauses: usize,
    warped: bool,
}

impl TutorialMode {
    pub fn new(ctx: &mut EventCtx, ui: &mut UI) -> Box<dyn State> {
        if ui.primary.map.get_name() != "montlake" {
            ui.switch_map(ctx, abstutil::path_map("montlake"));
        }

        let mut tut = TutorialState::new(ctx, ui);
        // For my sanity
        if ui.opts.dev {
            tut.latest = tut.stages.len() - 1;
            tut.current = tut.latest;
        }
        tut.make_state(ctx, ui)
    }
}

impl State for TutorialMode {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        // First of all, might need to initiate warping
        if !self.warped {
            match self.state.stage() {
                Stage::Msg { ref warp_pt, .. } | Stage::Interact { ref warp_pt, .. } => {
                    if let Some(id) = warp_pt {
                        self.warped = true;
                        return Transition::Push(Warping::new(
                            ctx,
                            id.canonical_point(&ui.primary).unwrap(),
                            Some(4.0),
                            Some(id.clone()),
                            &mut ui.primary,
                        ));
                    }
                }
            }
        }

        match self.top_center.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Quit" => {
                    ui.primary.clear_sim();
                    ui.overlay = Overlays::Inactive;
                    return Transition::Pop;
                }
                "<" => {
                    self.state.current -= 1;
                    return Transition::Replace(self.state.make_state(ctx, ui));
                }
                ">" => {
                    self.state.current += 1;
                    return Transition::Replace(self.state.make_state(ctx, ui));
                }
                _ => unreachable!(),
            },
            None => {}
        }

        if let Some(ref mut msg) = self.msg_panel {
            match msg.event(ctx) {
                Some(Outcome::Clicked(x)) => match x.as_ref() {
                    "OK" => {
                        self.state.next();
                        if self.state.current == self.state.stages.len() {
                            ui.primary.clear_sim();
                            return Transition::Pop;
                        } else {
                            return Transition::Replace(self.state.make_state(ctx, ui));
                        }
                    }
                    _ => unreachable!(),
                },
                None => {
                    // Don't allow other interactions
                    return Transition::Keep;
                }
            }
        }

        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }

        if let Some(ref mut tp) = self.time_panel {
            tp.event(ctx, ui);
        }

        if let Some(ref mut speed) = self.speed {
            match speed.event(ctx, ui) {
                Some(WrappedOutcome::Transition(t)) => {
                    return t;
                }
                Some(WrappedOutcome::Clicked(x)) => match x {
                    x if x == "reset to midnight" => {
                        return Transition::Replace(self.state.make_state(ctx, ui));
                    }
                    _ => unreachable!(),
                },
                None => {}
            }
        }
        if let Some(ref mut am) = self.agent_meter {
            if let Some(t) = am.event(ctx, ui) {
                return t;
            }
        }
        if let Some(ref mut m) = self.minimap {
            if let Some(t) = m.event(ui, ctx) {
                return t;
            }
            if let Some(t) = Overlays::update(ctx, ui) {
                return t;
            }
        }

        // Interaction things
        let interact = self.state.interaction();
        if interact == "Put out the fire at the Montlake Market" {
            if ui.primary.current_selection == Some(ID::Building(BuildingID(9)))
                && ui.per_obj.left_click(ctx, "put out the... fire?")
            {
                self.state.next();
                return Transition::Replace(self.state.make_state(ctx, ui));
            }
        } else if interact == "Inspect a lane, intersection, and building" {
            match ui.primary.current_selection {
                Some(ID::Lane(_)) => {
                    if !self.inspected_lane && ui.per_obj.action(ctx, Key::I, "inspect the lane") {
                        self.inspected_lane = true;
                        return Transition::Push(msg(
                            "Inspection",
                            vec!["Yup, it's a lane belonging to a road, alright."],
                        ));
                    }
                }
                Some(ID::Building(_)) => {
                    if !self.inspected_building
                        && ui.per_obj.action(ctx, Key::I, "inspect the building")
                    {
                        self.inspected_building = true;
                        return Transition::Push(msg(
                            "Inspection",
                            vec![
                                "Knock knock, anyone home?",
                                "Did you know: most trips begin and end at a building.",
                            ],
                        ));
                    }
                }
                Some(ID::Intersection(_)) => {
                    if !self.inspected_intersection
                        && ui.per_obj.action(ctx, Key::I, "inspect the intersection")
                    {
                        self.inspected_intersection = true;
                        return Transition::Push(msg(
                            "Inspection",
                            vec!["Insert clever quip about intersections here"],
                        ));
                    }
                }
                _ => {}
            }
            if self.inspected_lane && self.inspected_building && self.inspected_intersection {
                self.state.next();
                return Transition::Replace(self.state.make_state(ctx, ui));
            }
        } else if interact == "Wait until 5pm" {
            if ui.primary.sim.time() >= Time::START_OF_DAY + Duration::hours(17) {
                self.state.next();
                return Transition::Replace(self.state.make_state(ctx, ui));
            }
        } else if interact == "Pause/resume 3 times" {
            if self.was_paused && !self.speed.as_ref().unwrap().is_paused() {
                self.was_paused = false;
            }
            if !self.was_paused && self.speed.as_ref().unwrap().is_paused() {
                self.num_pauses += 1;
                self.was_paused = true;
            }
            if self.num_pauses == 3 {
                self.state.next();
                return Transition::Replace(self.state.make_state(ctx, ui));
            }
        } else if interact == "Escort the first northbound car until they park" {
            if let Some(ID::Car(c)) = ui.primary.current_selection {
                if ui.per_obj.action(ctx, Key::C, "check the car") {
                    if c == CarID(19, VehicleType::Car) {
                        if ui.primary.sim.agent_to_trip(AgentID::Car(c)).is_some() {
                            return Transition::Push(msg(
                                "Not yet!",
                                vec![
                                    "The car is still traveling somewhee.",
                                    "Wait for the car to park. (You can speed up time!)",
                                ],
                            ));
                        } else {
                            self.state.next();
                            return Transition::Replace(self.state.make_state(ctx, ui));
                        }
                    } else {
                        return Transition::Push(msg(
                            "Wrong car",
                            vec![
                                "You're looking at the wrong car.",
                                "Use the 'reset to midnight' (key binding 'X') to start over, if \
                                 you lost the car to follow.",
                            ],
                        ));
                    }
                }
            }
        } else if interact == "Find a road with almost no parking spots available" {
            if let Some(ID::Lane(l)) = ui.primary.current_selection {
                if ui
                    .per_obj
                    .action(ctx, Key::C, "check the parking availability")
                {
                    let lane = ui.primary.map.get_l(l);
                    if !lane.is_parking() {
                        return Transition::Push(msg(
                            "Uhh..",
                            vec!["That's not even a parking lane"],
                        ));
                    }
                    let percent = (ui.primary.sim.get_free_spots(l).len() as f64)
                        / (lane.number_parking_spots() as f64);
                    if percent > 0.1 {
                        return Transition::Push(msg(
                            "Not quite",
                            vec![
                                format!("This lane has {:.0}% spots free", percent * 100.0),
                                "Try using the 'parking availability' layer from the minimap \
                                 controls"
                                    .to_string(),
                            ],
                        ));
                    }
                    self.state.next();
                    return Transition::Replace(self.state.make_state(ctx, ui));
                }
            }
        }

        if let Some(ref mut common) = self.common {
            if let Some(t) = common.event(ctx, ui, self.speed.as_mut()) {
                return t;
            }
        }

        if self.speed.as_ref().map(|s| s.is_paused()).unwrap_or(true) {
            Transition::Keep
        } else {
            Transition::KeepWithMode(EventLoopMode::Animation)
        }
    }

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        ui.draw(
            g,
            self.common
                .as_ref()
                .map(|c| c.draw_options(ui))
                .unwrap_or_else(DrawOptions::new),
            &ui.primary.sim,
            &ShowEverything::new(),
        );
        ui.overlay.draw(g);
        if self.msg_panel.is_some() {
            // Make it clear the map can't be interacted with right now.
            g.fork_screenspace();
            // TODO - OSD height
            g.draw_polygon(
                Color::BLACK.alpha(0.5),
                &Polygon::rectangle(g.canvas.window_width, g.canvas.window_height),
            );
            g.unfork();
        }

        self.top_center.draw(g);

        if let Some(ref msg) = self.msg_panel {
            msg.draw(g);
        }
        if let Some(ref time) = self.time_panel {
            time.draw(g);
        }
        if let Some(ref speed) = self.speed {
            speed.draw(g);
        }
        if let Some(ref am) = self.agent_meter {
            am.draw(g);
        }
        if let Some(ref m) = self.minimap {
            m.draw(g, ui);
        }
        if let Some(ref common) = self.common {
            common.draw(g, ui);
        }

        // Special things
        if self.state.interaction() == "Put out the fire at the Montlake Market" {
            g.draw_polygon(Color::RED, &ui.primary.map.get_b(BuildingID(9)).polygon);
        }

        if let Stage::Msg { point_to, .. } = self.state.stage() {
            if let Some(fxn) = point_to {
                let pt = (fxn)(g, ui);
                g.fork_screenspace();
                g.draw_polygon(
                    Color::RED,
                    &PolyLine::new(vec![g.canvas.center_to_screen_pt().to_pt(), pt])
                        .make_arrow(Distance::meters(20.0))
                        .unwrap(),
                );
                g.unfork();
            }
        }
    }
}

enum Stage {
    Msg {
        lines: Vec<&'static str>,
        point_to: Option<Box<dyn Fn(&GfxCtx, &UI) -> Pt2D>>,
        warp_pt: Option<ID>,
        spawn_around: Option<IntersectionID>,
    },
    Interact {
        name: &'static str,
        warp_pt: Option<ID>,
        spawn_around: Option<IntersectionID>,
        spawn_randomly: bool,
    },
}

impl Stage {
    fn msg(lines: Vec<&'static str>) -> Stage {
        Stage::Msg {
            lines,
            point_to: None,
            warp_pt: None,
            spawn_around: None,
        }
    }

    fn interact(name: &'static str) -> Stage {
        Stage::Interact {
            name,
            warp_pt: None,
            spawn_around: None,
            spawn_randomly: false,
        }
    }

    fn arrow(self, pt: ScreenPt) -> Stage {
        self.dynamic_arrow(Box::new(move |_, _| pt.to_pt()))
    }
    fn dynamic_arrow(mut self, cb: Box<dyn Fn(&GfxCtx, &UI) -> Pt2D>) -> Stage {
        match self {
            Stage::Msg {
                ref mut point_to, ..
            } => {
                assert!(point_to.is_none());
                *point_to = Some(cb);
                self
            }
            Stage::Interact { .. } => unreachable!(),
        }
    }

    fn warp_to(mut self, id: ID) -> Stage {
        match self {
            Stage::Msg {
                ref mut warp_pt, ..
            } => {
                assert!(warp_pt.is_none());
                *warp_pt = Some(id);
                self
            }
            Stage::Interact {
                ref mut warp_pt, ..
            } => {
                assert!(warp_pt.is_none());
                *warp_pt = Some(id);
                self
            }
        }
    }

    fn spawn_around(mut self, i: IntersectionID) -> Stage {
        match self {
            Stage::Msg {
                ref mut spawn_around,
                ..
            } => {
                assert!(spawn_around.is_none());
                *spawn_around = Some(i);
                self
            }
            Stage::Interact {
                ref mut spawn_around,
                ..
            } => {
                assert!(spawn_around.is_none());
                *spawn_around = Some(i);
                self
            }
        }
    }

    fn spawn_randomly(mut self) -> Stage {
        match self {
            Stage::Msg { .. } => unreachable!(),
            Stage::Interact {
                ref mut spawn_randomly,
                ref spawn_around,
                ..
            } => {
                assert!(!*spawn_randomly);
                assert!(spawn_around.is_none());
                *spawn_randomly = true;
                self
            }
        }
    }
}

// TODO Ideally we'd replace self, not clone.
struct TutorialState {
    stages: Vec<Stage>,
    latest: usize,
    current: usize,
}

impl TutorialState {
    fn stage(&self) -> &Stage {
        &self.stages[self.current]
    }

    fn interaction(&self) -> String {
        match self.stage() {
            Stage::Msg { .. } => String::new(),
            Stage::Interact { ref name, .. } => name.to_string(),
        }
    }

    fn next(&mut self) {
        self.current += 1;
        self.latest = self.latest.max(self.current);
    }

    fn make_top_center(&self, ctx: &mut EventCtx) -> Composite {
        let mut col = vec![ManagedWidget::row(vec![
            ManagedWidget::draw_text(ctx, Text::from(Line("Tutorial").size(26))).margin(5),
            ManagedWidget::draw_batch(
                ctx,
                GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 50.0))]),
            )
            .margin(5),
            ManagedWidget::draw_text(
                ctx,
                Text::from(Line(format!("{}/{}", self.current + 1, self.stages.len())).size(20)),
            )
            .margin(5),
            if self.current == 0 {
                ManagedWidget::draw_text(ctx, Text::from(Line("<")))
            } else {
                WrappedComposite::text_button(ctx, "<", None)
            },
            if self.current == self.latest {
                ManagedWidget::draw_text(ctx, Text::from(Line(">")))
            } else {
                WrappedComposite::text_button(ctx, ">", None)
            },
            WrappedComposite::text_button(ctx, "Quit", None),
        ])
        .centered()];
        if let Stage::Interact { name, .. } = self.stage() {
            col.push(ManagedWidget::draw_text(ctx, Text::from(Line(*name))));
        }

        Composite::new(ManagedWidget::col(col).bg(Color::grey(0.4)))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx)
    }

    fn make_state(&self, ctx: &mut EventCtx, ui: &mut UI) -> Box<dyn State> {
        if let Stage::Msg { .. } = self.stage() {
            ui.primary.current_selection = None;
        }

        ui.primary.clear_sim();
        ui.overlay = Overlays::Inactive;
        if let Some(i) = match self.stage() {
            Stage::Msg {
                ref spawn_around, ..
            } => spawn_around,
            Stage::Interact {
                ref spawn_around, ..
            } => spawn_around,
        } {
            let old = ui.primary.current_flags.sim_flags.rng_seed;
            ui.primary.current_flags.sim_flags.rng_seed = Some(42);
            spawn_agents_around(*i, ui, ctx);
            ui.primary.current_flags.sim_flags.rng_seed = old;
        }
        if match self.stage() {
            Stage::Msg { .. } => false,
            Stage::Interact { spawn_randomly, .. } => *spawn_randomly,
        } {
            Scenario::small_run(&ui.primary.map).instantiate(
                &mut ui.primary.sim,
                &ui.primary.map,
                &mut ui.primary.current_flags.sim_flags.make_rng(),
                &mut Timer::throwaway(),
            );
            ui.primary.sim.step(&ui.primary.map, Duration::seconds(0.1));
        }

        // Ew, this is brittle.
        let mut num_interacts = 0;
        // Don't count the current.
        for stage in &self.stages[0..self.current] {
            if let Stage::Interact { .. } = stage {
                num_interacts += 1;
            }
        }

        // TODO Expensive
        let mut state = TutorialState::new(ctx, ui);
        state.current = self.current;
        state.latest = self.latest;
        Box::new(TutorialMode {
            state,

            top_center: self.make_top_center(ctx),

            msg_panel: match self.stage() {
                Stage::Msg { ref lines, .. } => Some(
                    Composite::new(
                        ManagedWidget::col(vec![
                            ManagedWidget::draw_text(ctx, {
                                let mut txt = Text::new();
                                for l in lines {
                                    txt.add(Line(*l));
                                }
                                txt
                            }),
                            ManagedWidget::row(vec![WrappedComposite::text_button(
                                ctx,
                                "OK",
                                hotkey(Key::Enter),
                            )])
                            .centered(),
                        ])
                        .bg(Color::grey(0.4))
                        .outline(5.0, Color::WHITE)
                        .padding(5),
                    )
                    .aligned(HorizontalAlignment::Center, VerticalAlignment::Center)
                    .build(ctx),
                ),
                Stage::Interact { .. } => None,
            },
            common: if num_interacts >= 1 {
                Some(CommonState::new())
            } else {
                None
            },
            time_panel: if num_interacts >= 2 {
                Some(TimePanel::new(ctx, ui))
            } else {
                None
            },
            speed: if num_interacts >= 2 {
                let mut speed = SpeedControls::new(ctx);
                speed.pause(ctx);
                Some(speed)
            } else {
                None
            },
            agent_meter: if num_interacts >= 4 {
                Some(AgentMeter::new(ctx, ui))
            } else {
                None
            },
            minimap: if num_interacts >= 5 {
                Some(Minimap::new(ctx, ui))
            } else {
                None
            },

            inspected_lane: false,
            inspected_building: false,
            inspected_intersection: false,
            was_paused: true,
            num_pauses: 0,
            warped: false,
        })
    }

    fn new(ctx: &mut EventCtx, ui: &mut UI) -> TutorialState {
        let time = TimePanel::new(ctx, ui);
        let speed = SpeedControls::new(ctx);
        let agent_meter = AgentMeter::new(ctx, ui);
        let minimap = Minimap::new(ctx, ui);

        let mut stages = vec![Stage::msg(vec![
            "Welcome to your first day as a contract traffic engineer --",
            "like a paid assassin, but capable of making WAY more people cry.",
            "Warring factions in Seattle have brought you here.",
        ])
        .warp_to(ID::Intersection(IntersectionID(141)))];

        stages.extend(vec![
            Stage::msg(vec![
                "Let's start with the controls for your handy drone.",
                "Click and drag to pan around the map, and use your scroll wheel or touchpad to \
                 zoom.",
            ]),
            Stage::msg(vec![
                "Let's try that ou--",
                "WHOA THE MONTLAKE MARKET IS ON FIRE!",
                "GO CLICK ON IT, QUICK!",
            ]),
            Stage::msg(vec!["(Hint: Look around for an unusually red building)"]),
            // TODO Just zoom in sufficiently on it, maybe don't even click it yet.
            Stage::interact("Put out the fire at the Montlake Market"),
        ]);

        stages.extend(vec![
            Stage::msg(vec![
                "Er, sorry about that.",
                "Just a little joke we like to play on the new recruits.",
            ]),
            Stage::msg(vec![
                "Now, let's learn how to inspect and interact with objects in the map.",
                "Select something, then click on it.",
                "",
                "HINT: The bottom of the screen shows keyboard shortcuts.",
            ])
            .arrow(ScreenPt::new(
                0.5 * ctx.canvas.window_width,
                0.97 * ctx.canvas.window_height,
            )),
            Stage::msg(vec![
                "I wonder what kind of information is available for different objects? Let's find \
                 out!",
            ]),
            Stage::interact("Inspect a lane, intersection, and building"),
        ]);

        stages.extend(vec![
            Stage::msg(vec![
                "Inspection complete!",
                "",
                "You'll work day and night, watching traffic patterns unfold.",
            ])
            .arrow(time.composite.center_of_panel()),
            Stage::msg(vec!["You can pause or resume time"])
                .arrow(speed.composite.inner.center_of("pause")),
            Stage::msg(vec!["Speed things up"])
                .arrow(speed.composite.inner.center_of("600x speed")),
            Stage::msg(vec!["Advance time by certain amounts"])
                .arrow(speed.composite.inner.center_of("step forwards 1 hour")),
            Stage::msg(vec!["And reset to the beginning of the day"])
                .arrow(speed.composite.inner.center_of("reset to midnight")),
            Stage::msg(vec!["Let's try these controls out. Just wait until 5pm."]),
            Stage::interact("Wait until 5pm"),
        ]);

        stages.extend(vec![
            Stage::msg(vec!["Whew, that took a while! (Hopefully not though...)"]),
            Stage::msg(vec![
                "You might've figured it out already,",
                "But you'll be pausing/resuming time VERY frequently",
            ])
            .arrow(speed.composite.inner.center_of("pause")),
            Stage::msg(vec![
                "Again, most controls have a key binding shown at the bottom of the screen.",
                "Press SPACE to pause/resume time.",
            ])
            .arrow(ScreenPt::new(
                0.5 * ctx.canvas.window_width,
                0.97 * ctx.canvas.window_height,
            )),
            Stage::msg(vec![
                "Just reassure me and pause/resume time a few times, alright?",
            ]),
            Stage::interact("Pause/resume 3 times"),
        ]);

        stages.extend(vec![
            Stage::msg(vec!["Alright alright, no need to wear out your spacebar."]),
            // Don't center on where the agents are, be a little offset
            Stage::msg(vec![
                "Oh look, some people appeared!",
                "We've got pedestrians, bikes, and cars moving around now.",
            ])
            .warp_to(ID::Building(BuildingID(611)))
            .spawn_around(IntersectionID(247)),
            Stage::msg(vec![
                "You can see the number of them in the top-right corner.",
            ])
            .arrow(agent_meter.composite.center_of_panel())
            .spawn_around(IntersectionID(247)),
            Stage::msg(vec![
                "Why don't you follow the first northbound car to their destination,",
                "and see where they park?",
            ])
            .spawn_around(IntersectionID(247))
            .warp_to(ID::Building(BuildingID(611)))
            .dynamic_arrow(Box::new(|g, ui| {
                g.canvas
                    .map_to_screen(
                        ui.primary
                            .sim
                            .canonical_pt_for_agent(
                                AgentID::Car(CarID(19, VehicleType::Car)),
                                &ui.primary.map,
                            )
                            .unwrap(),
                    )
                    .to_pt()
            })),
            // TODO Make it clear they can reset
            Stage::interact("Escort the first northbound car until they park")
                .spawn_around(IntersectionID(247))
                .warp_to(ID::Building(BuildingID(611))),
        ]);

        stages.extend(vec![
            Stage::msg(vec![
                "Escort mission complete.",
                "",
                "The map is quite large, so to help you orient",
                "the minimap shows you an overview of all activity.",
            ])
            .arrow(minimap.composite.center_of("minimap")),
            Stage::msg(vec!["Find addresses here"]).arrow(minimap.composite.center_of("search")),
            Stage::msg(vec!["Set up shortcuts to favorite areas"])
                .arrow(minimap.composite.center_of("shortcuts")),
            Stage::msg(vec!["View different data about agents"])
                .arrow(minimap.composite.center_of("change agent colorscheme")),
            Stage::msg(vec!["Apply different heatmap layers to the map"])
                .arrow(minimap.composite.center_of("change overlay")),
            Stage::msg(vec![
                "Let's try these out.",
                "There are lots of cars parked everywhere.",
                "Can you find a road that's almost out of parking spots?",
            ]),
            Stage::interact("Find a road with almost no parking spots available").spawn_randomly(),
        ]);

        stages.push(Stage::msg(vec![
            "Training complete!",
            "Go have the appropriate amount of fun.",
        ]));

        TutorialState {
            stages,
            latest: 0,
            current: 0,
        }

        // You've got a drone and, thanks to extremely creepy surveillance technology, the ability
        // to peer into everyone's trips.
        // People are on fixed schedules: every day, they leave at exactly the same time using the
        // same mode of transport. All you can change is how their experience will be in the
        // short-term. The city is in total crisis. You've only got 10 days to do something
        // before all hell breaks loose and people start kayaking / ziplining / crab-walking
        // / cartwheeling / to work.

        // TODO Show overlapping peds?
        // TODO Multi-modal trips -- including parking. (Cars per bldg, ownership). Border
        // intersections.

        // TODO Edit mode. fixed schedules. agenda/goals.
        // - add a bike lane, watch cars not stack up anymore
        // - Traffic signals -- protected and permited turns
        // - buses... bus lane to skip traffic, reduce waiting time.

        // TODO Misc tools -- shortcuts, find address
    }
}
