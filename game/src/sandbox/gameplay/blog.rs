use map_gui::tools::nice_map_name;
use widgetry::{
    lctrl, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, StyledButtons,
    VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::edit::EditMode;
use crate::sandbox::gameplay::freeform::ChangeScenario;
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::{Actions, SandboxControls};

pub struct Blog {
    top_center: Panel,
}

impl Blog {
    pub fn new(ctx: &mut EventCtx) -> Box<dyn GameplayState> {
        Box::new(Blog {
            top_center: Panel::empty(ctx),
        })
    }
}

impl GameplayState for Blog {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        _: &mut SandboxControls,
        _: &mut Actions,
    ) -> Option<Transition> {
        match self.top_center.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                // TODO This'll bring us out of this GameplayMode.
                "change scenario" => Some(Transition::Push(ChangeScenario::new(ctx, app, "none"))),
                "edit map" => Some(Transition::Push(EditMode::new(
                    ctx,
                    app,
                    GameplayMode::Freeform(app.primary.map.get_name().clone()),
                ))),
                _ => unreachable!(),
            },
            _ => None,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_center.draw(g);
    }

    fn recreate_panels(&mut self, ctx: &mut EventCtx, app: &App) {
        let row = Widget::row(vec![
            Line(nice_map_name(app.primary.map.get_name()))
                .small_heading()
                .draw(ctx),
            Widget::vert_separator(ctx, 50.0),
            ctx.style()
                .btn_light_popup_icon_text("system/assets/tools/calendar.svg", "none")
                .hotkey(Key::S)
                .build_widget(ctx, "change scenario"),
            ctx.style()
                .btn_outline_light_icon_text("system/assets/tools/pencil.svg", "Edit map")
                .hotkey(lctrl(Key::E))
                .build_widget(ctx, "edit map"),
        ])
        .centered();

        self.top_center = Panel::new(row)
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx);
    }

    fn has_tool_panel(&self) -> bool {
        // Get rid of the home button, which would allow escaping to the title screen
        false
    }
}
