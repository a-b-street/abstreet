use crate::app::App;
use crate::challenges::Challenge;
use crate::edit::EditMode;
use crate::game::{msg, Transition};
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::sandbox::gameplay::{challenge_header, GameplayMode, GameplayState};
use crate::sandbox::SandboxControls;
use ezgui::{Composite, EventCtx, GfxCtx, HorizontalAlignment, VerticalAlignment, Widget};

pub struct CreateGridlock {
    top_center: WrappedComposite,
}

impl CreateGridlock {
    pub fn new(ctx: &mut EventCtx, app: &App, mode: GameplayMode) -> Box<dyn GameplayState> {
        Box::new(CreateGridlock {
            top_center: challenge_controller(ctx, app, mode, "Gridlock Challenge", Vec::new()),
        })
    }
}

impl GameplayState for CreateGridlock {
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

        (None, false)
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_center.draw(g);
    }
}

fn challenge_controller(
    ctx: &mut EventCtx,
    app: &App,
    gameplay: GameplayMode,
    title: &str,
    extra_rows: Vec<Widget>,
) -> WrappedComposite {
    let description = Challenge::find(&gameplay).0.description;

    let mut rows = vec![challenge_header(ctx, title)];
    rows.extend(extra_rows);

    WrappedComposite::new(
        Composite::new(Widget::col(rows).bg(app.cs.panel_bg))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
    )
    .cb(
        "edit map",
        Box::new(move |ctx, app| {
            Some(Transition::Push(Box::new(EditMode::new(
                ctx,
                app,
                gameplay.clone(),
            ))))
        }),
    )
    // TODO msg() is silly, it's hard to plumb the title. Also, show the challenge splash screen.
    .cb(
        "instructions",
        Box::new(move |_, _| Some(Transition::Push(msg("Challenge", description.clone())))),
    )
}
