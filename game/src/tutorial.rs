use crate::common::CommonState;
use crate::game::{msg, State, Transition};
use crate::helpers::ID;
use crate::sandbox::{SpeedControls, TimePanel};
use crate::ui::{ShowEverything, UI};
use ezgui::{
    Color, Composite, EventCtx, EventLoopMode, GfxCtx, HorizontalAlignment, Key, Line,
    ManagedWidget, Outcome, Text, VerticalAlignment,
};
use geom::{Duration, Time};
use map_model::{BuildingID, LaneID};
use std::collections::HashSet;

// You're a contract traffic engineer -- like a paid assassin, but capable of making way more
// people cry. Warring factions in Seattle have brought you here.
// You've got a drone and, thanks to extremely creepy surveillance technology, the ability to peer
// into everyone's trips.
// People are on fixed schedules: every day, they leave at exactly the same time using the same
// mode of transport. All you can change is how their experience will be in the short-term.
// The city is in total crisis. You've only got 10 days to do something before all hell breaks
// loose and people start kayaking / ziplining / crab-walking / cartwheeling / to work.

pub enum Stage {
    // Pan and zoom. Go find some obvious landmark.
    // TODO Just zoom in sufficiently on it, maybe don't even click it yet.
    CanvasControls,

    // Select objects, use info panel, OSD, hotkeys. Measure the length of some roads, do
    // action on 3 roads.
    // (use big arrows to point to stuff)
    SelectObjects(HashSet<LaneID>),

    // Time, Speed panels. They don't do much yet. Wait until some time.
    TimeControls,

    // TODO Spawn agents at an intersection. Point out different vehicles. Show overlapping peds. Silently introduce agent meters panel. Tell people to go check out the destination of some particular car (click on it).
    // TODO Minimap, layers
    // TODO Multi-modal trips -- including parking. (Cars per bldg, ownership). Border intersections.

    // TODO Edit mode. fixed schedules. agenda/goals.
    // - add a bike lane, watch cars not stack up anymore
    // - Traffic signals -- protected and permited turns
    // - buses... bus lane to skip traffic, reduce waiting time.

    // TODO Misc tools -- shortcuts, find address
    End,
}

pub struct TutorialMode {
    stage: Stage,
    instructions: Composite,

    common: CommonState,
    time_panel: TimePanel,
    speed: SpeedControls,
}

impl TutorialMode {
    pub fn new(ctx: &mut EventCtx, ui: &UI, stage: Stage) -> TutorialMode {
        let mut txt = Text::from(Line("Tutorial mode"));
        txt.highlight_last_line(Color::BLUE);
        match stage {
            Stage::CanvasControls => {
                txt.add(Line("Welcome to your first da--"));
                txt.add(Line("AHHH THE SPACE NEEDLE IS ON FIRE!!!"));
                txt.add(Line("GO CLICK ON IT!"));
                txt.add(Line(
                    "(Click and drag to pan the map, scroll up/down to zoom.)",
                ));
            }
            Stage::SelectObjects(_) => {
                txt.add(Line(
                    "Er, sorry about that. Just a little joke we like to play on the new recruits.",
                ));
                txt.add(Line("Let's learn how to inspect objects."));
                txt.add(Line("Select something, then click on it."));
                txt.add(Line("The bottom of the screen shows keyboard shortcuts."));
                txt.add(Line(""));
                txt.add(Line(
                    "Almost time to hit the road! Go hit 3 different lanes on one road.",
                ));
            }
            Stage::TimeControls => {
                txt.add(Line(
                    "You'll work day and night, watching traffic patterns unfold.",
                ));
                // TODO Point to individual buttons
                txt.add(Line("Use the speed controls to pause time, speed things up, or reset to the beginning of the day."));
                txt.add(Line(""));
                txt.add(Line("Wait until 5pm to proceed (or skip to that point!"));
            }
            Stage::End => {
                txt.add(Line("Well done! Now go pick a challenge to work on."));
            }
        };

        // TODO Start in one fixed area of a particular map

        let mut speed = SpeedControls::new(ctx);
        speed.pause(ctx);
        TutorialMode {
            stage,
            // TODO Back button, part X/Y
            instructions: Composite::new(
                ManagedWidget::row(vec![
                    ManagedWidget::draw_text(ctx, txt),
                    crate::managed::Composite::text_button(ctx, "Quit tutorial", None),
                ])
                .bg(Color::grey(0.4)),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),

            common: CommonState::new(),
            time_panel: TimePanel::new(ctx, ui),
            speed,
        }
    }
}

impl State for TutorialMode {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        match self.instructions.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Quit tutorial" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        // Always
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }

        match self.stage {
            Stage::CanvasControls => {
                // TODO Not the space needle, obviously
                if ui.primary.current_selection == Some(ID::Building(BuildingID(9)))
                    && ui.per_obj.left_click(ctx, "put out the... fire?")
                {
                    return Transition::Replace(Box::new(TutorialMode::new(
                        ctx,
                        ui,
                        Stage::SelectObjects(HashSet::new()),
                    )));
                }
            }
            Stage::SelectObjects(ref mut hit) => {
                if let Some(ID::Lane(l)) = ui.primary.current_selection {
                    if !hit.contains(&l) && ui.per_obj.action(ctx, Key::H, "hit the road") {
                        hit.insert(l);
                        if hit.len() == 3 {
                            return Transition::Replace(Box::new(TutorialMode::new(
                                ctx,
                                ui,
                                Stage::TimeControls,
                            )));
                        } else {
                            return Transition::Push(msg(
                                "You hit the road",
                                vec![format!("Ouch! Poor road. {} more", 3 - hit.len())],
                            ));
                        }
                    }
                }
            }
            Stage::TimeControls => {
                self.time_panel.event(ctx, ui);

                match self.speed.event(ctx, ui) {
                    Some(crate::managed::Outcome::Transition(t)) => {
                        return t;
                    }
                    Some(crate::managed::Outcome::Clicked(x)) => match x {
                        x if x == "reset to midnight" => {
                            ui.primary.clear_sim();
                        }
                        _ => unreachable!(),
                    },
                    None => {}
                }

                if ui.primary.sim.time() >= Time::START_OF_DAY + Duration::hours(17) {
                    return Transition::Replace(Box::new(TutorialMode::new(ctx, ui, Stage::End)));
                }
            }
            Stage::End => {}
        }

        match self.stage {
            Stage::CanvasControls => {}
            _ => {
                if let Some(t) = self.common.event(ctx, ui) {
                    return t;
                }
            }
        }

        if self.speed.is_paused() {
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
            self.common.draw_options(ui),
            &ui.primary.sim,
            &ShowEverything::new(),
        );
        self.instructions.draw(g);

        match self.stage {
            Stage::CanvasControls => {}
            Stage::SelectObjects(_) => {
                self.common.draw(g, ui);
            }
            Stage::TimeControls => {
                self.common.draw(g, ui);
                self.time_panel.draw(g);
                self.speed.draw(g);
            }
            Stage::End => {
                self.common.draw(g, ui);
                self.time_panel.draw(g);
                self.speed.draw(g);
            }
        }
    }
}
