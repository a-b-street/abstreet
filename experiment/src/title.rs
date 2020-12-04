use map_gui::tools::{open_browser, PopupMsg};
use widgetry::{
    Btn, DrawBaselayer, EventCtx, GfxCtx, Key, Line, Outcome, Panel, State, Text, Widget,
};

use crate::levels::Level;
use crate::{App, Transition};

pub struct TitleScreen {
    panel: Panel,
}

impl TitleScreen {
    pub fn new(ctx: &mut EventCtx) -> Box<dyn State<App>> {
        let levels = Level::all();

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
                    Widget::row(
                        levels
                            .into_iter()
                            .map(|lvl| Btn::text_bg2(lvl.title).build_def(ctx, None))
                            .collect(),
                    ),
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
                    // TODO As I'm writing the range argument, I don't buy the hybrid motor.
                    // Wireless Tesla energy instead?
                    return Transition::Push(PopupMsg::new(
                        ctx,
                        "Instructions",
                        vec![
                            "It's Christmas Eve, so it's time for Santa to deliver presents in \
                             Seattle. 2020 has thoroughly squashed any remaining magic out of the \
                             world, so your sleigh can only hold so many presents at a time.",
                            "Deliver presents as fast as you can. When you run out, refill from a \
                             yellow-colored store.",
                            "It's faster to deliver to buildings with multiple families inside.",
                            "",
                            "When you deliver enough presents, a little bit of magic is restored, \
                             and you can upzone buildings to make your job easier.",
                            "If you're having trouble delivering to houses far away from \
                             businesses, why not build a new grocery store where it might be \
                             needed?",
                        ],
                    ));
                }
                x => {
                    if let Some(url) = x.strip_prefix("open ") {
                        open_browser(url.to_string());
                        return Transition::Keep;
                    }

                    for lvl in Level::all() {
                        if x == lvl.title {
                            return Transition::Push(crate::before_level::Picker::new(
                                ctx, app, lvl,
                            ));
                        }
                    }
                    panic!("Unknown action {}", x);
                }
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.dialog_bg);
        self.panel.draw(g);
    }
}
