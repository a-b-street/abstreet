use map_gui::tools::{open_browser, PopupMsg};
use widgetry::{
    Btn, DrawBaselayer, EventCtx, GfxCtx, Key, Line, Outcome, Panel, State, Text, Widget,
};

use crate::{App, Transition};

pub struct TitleScreen {
    panel: Panel,
}

impl TitleScreen {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut level_buttons = Vec::new();
        for (idx, level) in app.session.levels.iter().enumerate() {
            if idx < app.session.levels_unlocked {
                level_buttons.push(Btn::text_bg2(&level.title).build_def(ctx, None));
            } else {
                level_buttons.push(Btn::text_bg2(&level.title).inactive(ctx));
            }
        }

        let upgrades = Text::from_multiline(vec![
            Line(format!(
                "Vehicles unlocked: {}",
                app.session.vehicles_unlocked.join(", "),
            )),
            Line(format!(
                "Upzones unlocked: {}",
                app.session.upzones_unlocked
            )),
        ]);

        Box::new(TitleScreen {
            panel: Panel::new(
                Widget::col(vec![
                    Btn::svg_def("system/assets/pregame/quit.svg")
                        .build(ctx, "quit", Key::Escape)
                        .align_left(),
                    {
                        let mut txt = Text::from(Line("15 minute Santa").display_title());
                        txt.add(Line("Created by Dustin Carlino, Yuwen Li, & Michael Kirk"));
                        txt.draw(ctx).centered_horiz()
                    },
                    Btn::text_bg1("Santa character created by @parallaxcreativedesign").build(
                        ctx,
                        "open https://www.instagram.com/parallaxcreativedesign/",
                        None,
                    ),
                    Btn::text_bg2("Instructions").build_def(ctx, None),
                    Widget::row(level_buttons),
                    upgrades.draw(ctx),
                ])
                .evenly_spaced(),
            )
            .exact_size_percent(90, 85)
            .build_custom(ctx),
        })
    }
}

impl State<App> for TitleScreen {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "quit" => {
                    std::process::exit(0);
                }
                "Instructions" => {
                    return Transition::Push(PopupMsg::new(
                        ctx,
                        "Instructions",
                        vec![
                            "It's Christmas Eve, so it's time for Santa to deliver presents in \
                             Seattle.",
                            "2020 has thoroughly squashed any remaining magic out of the world, \
                             so your sleigh can only hold so many presents at a time.",
                            "Deliver presents as fast as you can. When you run out, refill from a \
                             yellow-colored store.",
                            "It's faster to deliver to buildings with multiple families inside.",
                        ],
                    ));
                }
                x => {
                    if let Some(url) = x.strip_prefix("open ") {
                        open_browser(url.to_string());
                        return Transition::Keep;
                    }

                    for level in &app.session.levels {
                        if x == level.title {
                            return Transition::Push(crate::before_level::Picker::new(
                                ctx,
                                app,
                                level.clone(),
                            ));
                        }
                    }
                    panic!("Unknown action {}", x);
                }
            },
            _ => {}
        }

        app.session.update_music(ctx);

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.dialog_bg);
        self.panel.draw(g);
        app.session.music.draw(g);
    }
}
