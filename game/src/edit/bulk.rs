use crate::app::{App, ShowEverything};
use crate::game::{State, Transition};
use crate::helpers::ID;
use ezgui::{
    hotkey, Btn, Composite, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, TextExt,
    VerticalAlignment, Widget,
};
use map_model::IntersectionID;
use sim::DontDrawAgents;

// TODO For now, individual turns can't be manipulated. Banning turns could be useful, but I'm not
// sure what to do about the player orphaning a section of the map.
pub struct BulkSelect {
    composite: Composite,
    i1: Option<IntersectionID>,
}

impl BulkSelect {
    pub fn new(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State> {
        app.primary.current_selection = None;
        Box::new(BulkSelect {
            composite: Composite::new(
                Widget::col(vec![
                    Line("Edit many roads").small_heading().draw(ctx),
                    "Click one intersection to start".draw_text(ctx),
                    Btn::text_fg("Quit")
                        .build_def(ctx, hotkey(Key::Escape))
                        .margin_above(10),
                ])
                .bg(app.cs.panel_bg)
                .padding(10),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            i1: None,
        })
    }
}

impl State for BulkSelect {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.primary.current_selection = app.calculate_current_selection(
                ctx,
                &DontDrawAgents {},
                &ShowEverything::new(),
                false,
                true,
            );
            if let Some(ID::Intersection(_)) = app.primary.current_selection {
            } else {
                app.primary.current_selection = None;
            }
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Quit" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.composite.draw(g);
    }
}
