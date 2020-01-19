use crate::common::CommonState;
use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::ui::{ShowEverything, UI};
use ezgui::{
    Color, Composite, EventCtx, GfxCtx, HorizontalAlignment, Line, ManagedWidget, Outcome, Text,
    VerticalAlignment,
};
use map_model::BuildingID;

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
    CanvasControls,

    // TODO Select objects, use info panel, OSD, hotkeys. Measure the length of some roads, do
    // action on 3 roads.
    // (use big arrows to point to stuff)
    // TODO Time, Speed panels. They don't do much yet. Wait until some time.

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
}

impl TutorialMode {
    pub fn new(ctx: &mut EventCtx, stage: Stage) -> TutorialMode {
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
            // ehem. sorry about that. just a little joke we like to play on the new recruits.
            Stage::End => {
                txt.add(Line("Well done! Now go pick a challenge to work on."));
            }
        };

        // TODO Start in one fixed area

        TutorialMode {
            stage,
            // TODO Back button
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
        }
    }
}

impl State for TutorialMode {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }

        match self.instructions.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Quit tutorial" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        match self.stage {
            Stage::CanvasControls => {
                // TODO Not the space needle, obviously
                if ui.primary.current_selection == Some(ID::Building(BuildingID(9)))
                    && ui.per_obj.left_click(ctx, "put out the... fire?")
                {
                    return Transition::Replace(Box::new(TutorialMode::new(ctx, Stage::End)));
                }
            }
            Stage::End => {}
        }

        // TODO Not in first stage
        if let Some(t) = self.common.event(ctx, ui) {
            return t;
        }

        Transition::Keep
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
        // TODO Not in first stage
        self.common.draw(g, ui);
        self.instructions.draw(g);
    }
}
