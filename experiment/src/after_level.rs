use abstutil::prettyprint_usize;
use map_gui::SimpleApp;
use widgetry::{
    Btn, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, Text, Transition,
    VerticalAlignment, Widget,
};

use crate::levels::Level;
use crate::session::Session;

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

        // TODO Store in app
        let mut session = Session::new();
        session.record_score(level.title, score);

        let mut txt = Text::new();
        txt.add(Line(format!("Results for {}", level.title)).small_heading());
        txt.add(Line(format!(
            "You delivered {} presents in {}. Your goal was {}",
            prettyprint_usize(score),
            level.time_limit,
            prettyprint_usize(level.goal)
        )));
        txt.add(Line(""));
        txt.add(Line("High scores:"));
        for (idx, score) in session.high_scores[level.title].iter().enumerate() {
            txt.add(Line(format!("{}) {}", idx + 1, prettyprint_usize(*score))));
        }

        let panel = Panel::new(Widget::col(vec![
            txt.draw(ctx),
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
