use crate::common::{CommonState, Warping};
use crate::game::{msg, State, Transition};
use crate::helpers::ID;
use crate::render::DrawOptions;
use crate::sandbox::{spawn_agents_around, AgentMeter, SpeedControls, TimePanel};
use crate::ui::{ShowEverything, UI};
use ezgui::{
    hotkey, Color, Composite, EventCtx, EventLoopMode, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, ManagedWidget, Outcome, Text, VerticalAlignment,
};
use geom::{Distance, Duration, PolyLine, Polygon, Pt2D, Time};
use map_model::{BuildingID, IntersectionID, LaneID};
use sim::{TripID, TripResult};
use std::collections::HashSet;

pub struct TutorialMode {
    state: TutorialState,

    top_center: Composite,

    msg_panel: Option<Composite>,
    common: Option<CommonState>,
    time_panel: Option<TimePanel>,
    speed: Option<SpeedControls>,
    agent_meter: Option<AgentMeter>,

    // Goofy state for just some stages.
    hit_roads: HashSet<LaneID>,
    warped: bool,
}

impl TutorialMode {
    pub fn new(ctx: &mut EventCtx, ui: &mut UI) -> Box<dyn State> {
        if ui.primary.map.get_name() != "montlake" {
            ui.switch_map(ctx, abstutil::path_map("montlake"));
        }

        let mut tut = TutorialState::new();
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
            let maybe_id = if self.state.current == 0 {
                Some(ID::Intersection(IntersectionID(141)))
            } else if self.state.current == 8 {
                // Don't center on where the agents are, be a little offset
                Some(ID::Building(BuildingID(611)))
            } else {
                None
            };
            if let Some(id) = maybe_id {
                self.warped = true;
                return Transition::PushWithMode(
                    Warping::new(
                        ctx,
                        id.canonical_point(&ui.primary).unwrap(),
                        Some(4.0),
                        Some(id),
                        &mut ui.primary,
                    ),
                    EventLoopMode::Animation,
                );
            }
        }

        match self.top_center.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Quit" => {
                    ui.primary.clear_sim();
                    return Transition::Pop;
                }
                "<" => {
                    self.state.current -= 1;
                    return Transition::ReplaceWithMode(
                        self.state.make_state(ctx, ui),
                        EventLoopMode::Animation,
                    );
                }
                ">" => {
                    self.state.current += 1;
                    return Transition::ReplaceWithMode(
                        self.state.make_state(ctx, ui),
                        EventLoopMode::Animation,
                    );
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
                            return Transition::ReplaceWithMode(
                                self.state.make_state(ctx, ui),
                                EventLoopMode::Animation,
                            );
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
                Some(crate::managed::Outcome::Transition(t)) => {
                    return t;
                }
                Some(crate::managed::Outcome::Clicked(x)) => match x {
                    x if x == "reset to midnight" => {
                        return Transition::ReplaceWithMode(
                            self.state.make_state(ctx, ui),
                            EventLoopMode::Animation,
                        );
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

        // Interaction things
        // TODO Maybe have callbacks for these?
        if self.state.current == 3 {
            if ui.primary.current_selection == Some(ID::Building(BuildingID(9)))
                && ui.per_obj.left_click(ctx, "put out the... fire?")
            {
                self.state.next();
                return Transition::ReplaceWithMode(
                    self.state.make_state(ctx, ui),
                    EventLoopMode::Animation,
                );
            }
        } else if self.state.current == 5 {
            if let Some(ID::Lane(l)) = ui.primary.current_selection {
                if !self.hit_roads.contains(&l) && ui.per_obj.action(ctx, Key::H, "hit the road") {
                    self.hit_roads.insert(l);
                    if self.hit_roads.len() == 3 {
                        self.state.next();
                        return Transition::ReplaceWithMode(
                            self.state.make_state(ctx, ui),
                            EventLoopMode::Animation,
                        );
                    } else {
                        return Transition::Push(msg(
                            "You hit the road",
                            vec![format!(
                                "Ouch! Poor road. {} more",
                                3 - self.hit_roads.len()
                            )],
                        ));
                    }
                }
            }
        } else if self.state.current == 7 {
            if ui.primary.sim.time() >= Time::START_OF_DAY + Duration::hours(17) {
                self.state.next();
                return Transition::ReplaceWithMode(
                    self.state.make_state(ctx, ui),
                    EventLoopMode::Animation,
                );
            }
        } else if self.state.current == 10 {
            if ui.primary.current_selection == Some(ID::Building(BuildingID(2322)))
                && ui.per_obj.action(ctx, Key::C, "check the house")
            {
                match ui.primary.sim.trip_to_agent(TripID(24)) {
                    TripResult::TripDone => {
                        self.state.next();
                        return Transition::ReplaceWithMode(
                            self.state.make_state(ctx, ui),
                            EventLoopMode::Animation,
                        );
                    }
                    _ => {
                        return Transition::Push(msg(
                            "Not yet!",
                            vec![
                                "The house is empty.",
                                "Wait for the car and passenger to arrive!",
                            ],
                        ));
                    }
                }
            }
        }

        if let Some(ref mut common) = self.common {
            if let Some(t) = common.event(ctx, ui) {
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
        if let Some(ref common) = self.common {
            common.draw(g, ui);
        }

        // Special things
        // TODO Maybe have callbacks for these?
        if self.state.current == 3 {
            g.draw_polygon(Color::RED, &ui.primary.map.get_b(BuildingID(9)).polygon);
        } else if self.state.current == 4 {
            // OSD
            point_to_onscreen(g, 0.5, 0.97);
        } else if self.state.current == 6 {
            // Time
            point_to_onscreen(g, 0.1, 0.15);
            // Speed
            point_to_onscreen(g, 0.5, 0.9);
        } else if self.state.current == 8 {
            // Agent meters
            point_to_onscreen(g, 0.8, 0.15);
        }
    }
}

fn point_to_onscreen(g: &mut GfxCtx, pct_width: f64, pct_height: f64) {
    g.fork_screenspace();
    g.draw_polygon(
        Color::RED,
        &PolyLine::new(vec![
            g.canvas.center_to_screen_pt().to_pt(),
            Pt2D::new(
                pct_width * g.canvas.window_width,
                pct_height * g.canvas.window_height,
            ),
        ])
        .make_arrow(Distance::meters(20.0))
        .unwrap(),
    );
    g.unfork();
}

#[derive(Clone)]
enum Stage {
    Msg(Vec<&'static str>),
    Interact(&'static str),
}

// TODO Ideally we'd replace self, not clone.
#[derive(Clone)]
struct TutorialState {
    stages: Vec<Stage>,
    latest: usize,
    current: usize,
}

impl TutorialState {
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
                crate::managed::Composite::text_button(ctx, "<", None)
            },
            if self.current == self.latest {
                ManagedWidget::draw_text(ctx, Text::from(Line(">")))
            } else {
                crate::managed::Composite::text_button(ctx, ">", None)
            },
            crate::managed::Composite::text_button(ctx, "Quit", None),
        ])
        .centered()];
        match &self.stages[self.current] {
            Stage::Msg(_) => {}
            Stage::Interact(instructions) => {
                col.push(ManagedWidget::draw_text(
                    ctx,
                    Text::from(Line(*instructions)),
                ));
            }
        }

        Composite::new(ManagedWidget::col(col).bg(Color::grey(0.4)))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx)
    }

    fn make_state(&self, ctx: &mut EventCtx, ui: &mut UI) -> Box<dyn State> {
        ui.primary.clear_sim();
        match &self.stages[self.current] {
            Stage::Msg(_) => {
                ui.primary.current_selection = None;
            }
            Stage::Interact(_) => {}
        }

        if self.current == 8 || self.current == 9 || self.current == 10 {
            let old = ui.primary.current_flags.sim_flags.rng_seed;
            ui.primary.current_flags.sim_flags.rng_seed = Some(42);
            spawn_agents_around(IntersectionID(247), ui, ctx);
            ui.primary.current_flags.sim_flags.rng_seed = old;
        }

        Box::new(TutorialMode {
            state: self.clone(),

            top_center: self.make_top_center(ctx),

            msg_panel: match &self.stages[self.current] {
                Stage::Msg(ref lines) => Some(
                    Composite::new(
                        ManagedWidget::col(vec![
                            ManagedWidget::draw_text(ctx, {
                                let mut txt = Text::new();
                                for l in lines {
                                    txt.add(Line(*l));
                                }
                                txt
                            }),
                            ManagedWidget::row(vec![crate::managed::Composite::text_button(
                                ctx,
                                "OK",
                                hotkey(Key::Enter),
                            )])
                            .centered(),
                        ])
                        .bg(Color::grey(0.4)),
                    )
                    .aligned(HorizontalAlignment::Center, VerticalAlignment::Center)
                    .build(ctx),
                ),
                Stage::Interact(_) => None,
            },
            common: if self.current >= 4 {
                Some(CommonState::new())
            } else {
                None
            },
            time_panel: if self.current >= 6 {
                Some(TimePanel::new(ctx, ui))
            } else {
                None
            },
            speed: if self.current >= 6 {
                let mut speed = SpeedControls::new(ctx);
                speed.pause(ctx);
                Some(speed)
            } else {
                None
            },
            agent_meter: if self.current >= 8 {
                Some(AgentMeter::new(ctx, ui))
            } else {
                None
            },

            hit_roads: HashSet::new(),
            warped: false,
        })
    }

    fn new() -> TutorialState {
        let stages = vec![
            // 0
            Stage::Msg(vec!["Welcome to your first day as a contract traffic engineer --", "like a paid assassin, but capable of making WAY more people cry.", "Warring factions in Seattle have brought you here."]),
            // 1
            Stage::Msg(vec!["Let's start with the controls for your handy drone.", "Click and drag to pan around the map, and use your scroll wheel or touchpad to zoom."]),
            // 2
            Stage::Msg(vec!["Let's try that ou--", "WHOA THE MONTLAKE MARKET IS ON FIRE!", "GO CLICK ON IT, QUICK!"]),
            // 3
            // TODO Just zoom in sufficiently on it, maybe don't even click it yet.
            Stage::Interact("Put out the fire at the Montlake Market"),

            // 4
            Stage::Msg(vec!["Er, sorry about that.", "Just a little joke we like to play on the new recruits.", "Now, let's learn how to inspect and interact with objects in the map.", "Select something, then click on it.", "", "HINT: The bottom of the screen shows keyboard shortcuts.", "", "Hmm, almost time to hit the road."]),
            // 5
            Stage::Interact("Go hit 3 different lanes on one road"),

            // 6
            Stage::Msg(vec!["You'll work day and night, watching traffic patterns unfold.", "Use the speed controls to pause time, speed things up, or reset to the beginning of the day."]),
            // 7
            Stage::Interact("Wait until 5pm"),

            // 8
            Stage::Msg(vec!["Oh look, some people appeared!", "We've got pedestrians, bikes, and cars moving around now.", "You can see the number of them in the top-right corner."]),
            // 9
            Stage::Msg(vec!["Why don't you follow the first northbound car to their destination,", "and make sure whoever gets out makes it inside their house safely?"]),
            // 10
            // TODO Make it clear they can reset
            // TODO The time controls are too jumpy; can we automatically slow down when
            // interesting stuff happens?
            Stage::Interact("Escort the first northbound car to their home"),

            // 11
            Stage::Msg(vec!["Training complete!", "Go have the appropriate amount of fun."]),
        ];
        TutorialState {
            stages,
            latest: 0,
            current: 0,
        }

        // You've got a drone and, thanks to extremely creepy surveillance technology, the ability to peer
        // into everyone's trips.
        // People are on fixed schedules: every day, they leave at exactly the same time using the same
        // mode of transport. All you can change is how their experience will be in the short-term.
        // The city is in total crisis. You've only got 10 days to do something before all hell breaks
        // loose and people start kayaking / ziplining / crab-walking / cartwheeling / to work.

        // TODO Show overlapping peds?
        // TODO Minimap, layers
        // TODO Multi-modal trips -- including parking. (Cars per bldg, ownership). Border intersections.

        // TODO Edit mode. fixed schedules. agenda/goals.
        // - add a bike lane, watch cars not stack up anymore
        // - Traffic signals -- protected and permited turns
        // - buses... bus lane to skip traffic, reduce waiting time.

        // TODO Misc tools -- shortcuts, find address
    }
}
