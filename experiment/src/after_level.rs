use abstutil::prettyprint_usize;
use map_gui::SimpleApp;
use widgetry::{
    Btn, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, TextExt,
    Transition, VerticalAlignment, Widget,
};

use crate::levels::Level;

const ZOOM: f64 = 2.0;

// TODO Display route, the buildings still undelivered, etc. Let the player strategize about how to
// do better.

pub struct Results {
    panel: Panel,
}

impl Results {
    pub fn new(
        ctx: &mut EventCtx,
        app: &SimpleApp,
        score: usize,
        level: &Level,
    ) -> Box<dyn State<SimpleApp>> {
        ctx.canvas.cam_zoom = ZOOM;
        ctx.canvas.center_on_map_pt(app.map.get_bounds().center());

        let panel = Panel::new(Widget::col(vec![
            Line(format!("Results for {}", level.title))
                .small_heading()
                .draw(ctx),
            format!(
                "You delivered {} presents in {}. Your goal was {}",
                prettyprint_usize(score),
                level.time_limit,
                prettyprint_usize(level.goal)
            )
            .draw_text(ctx),
            Btn::text_bg2("Back to title screen").build_def(ctx, Key::Enter),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx);
        Box::new(Results { panel })
    }
}

impl State<SimpleApp> for Results {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut SimpleApp) -> Transition<SimpleApp> {
        ctx.canvas_movement();

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Back to title screen" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &SimpleApp) {
        self.panel.draw(g);
    }
}
