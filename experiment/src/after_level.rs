use abstutil::prettyprint_usize;
use map_gui::tools::PopupMsg;
use widgetry::{
    Btn, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, Text,
    VerticalAlignment, Widget,
};

use crate::levels::Level;
use crate::title::TitleScreen;
use crate::{App, Transition};

const ZOOM: f64 = 2.0;

// TODO Display route, the buildings still undelivered, etc. Let the player strategize about how to
// do better.

pub struct Results {
    panel: Panel,
    unlock_messages: Option<Vec<String>>,
}

impl Results {
    pub fn new(
        ctx: &mut EventCtx,
        app: &mut App,
        score: usize,
        level: &Level,
    ) -> Box<dyn State<App>> {
        ctx.canvas.cam_zoom = ZOOM;
        ctx.canvas.center_on_map_pt(app.map.get_bounds().center());

        let unlock_messages = app.session.record_score(level.title.clone(), score);

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
        for (idx, score) in app.session.high_scores[&level.title].iter().enumerate() {
            txt.add(Line(format!("{}) {}", idx + 1, prettyprint_usize(*score))));
        }

        let panel = Panel::new(Widget::col(vec![
            txt.draw(ctx),
            Btn::text_bg2("Back to title screen").build_def(ctx, Key::Enter),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx);
        Box::new(Results {
            panel,
            unlock_messages,
        })
    }
}

impl State<App> for Results {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Back to title screen" => {
                    let mut transitions = vec![
                        Transition::Pop,
                        Transition::Replace(TitleScreen::new(ctx, app)),
                    ];
                    if let Some(msgs) = self.unlock_messages.take() {
                        transitions.push(Transition::Push(PopupMsg::new(
                            ctx,
                            "Level complete!",
                            msgs,
                        )));
                    }
                    return Transition::Multi(transitions);
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        app.session.update_music(ctx);

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        app.session.music.draw(g);
    }
}
