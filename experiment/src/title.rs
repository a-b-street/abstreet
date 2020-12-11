use geom::Percent;
use map_gui::tools::open_browser;
use widgetry::{
    Btn, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel, State, Text, TextExt, Widget,
};

use crate::levels::Level;
use crate::{App, Transition};

pub struct TitleScreen {
    panel: Panel,
}

impl TitleScreen {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut level_buttons = Vec::new();
        for (idx, level) in app.session.levels.iter().enumerate() {
            if idx < app.session.levels_unlocked {
                level_buttons.push(unlocked_level(ctx, app, level, idx));
            } else {
                level_buttons.push(locked_level(ctx, app, level, idx));
            }
        }

        Box::new(TitleScreen {
            panel: Panel::new(Widget::col(vec![
                Btn::svg_def("system/assets/tools/quit.svg")
                    .build(ctx, "quit", Key::Escape)
                    .align_right(),
                Line("A/B STREET")
                    .big_heading_plain()
                    .draw(ctx)
                    .centered_horiz(),
                Line("15 Minute Santa")
                    .display_title()
                    .draw(ctx)
                    .centered_horiz(),
                Text::from(Line(
                    "Time for Santa to deliver presents in Seattle! But... 2020 has thoroughly \
                     squashed any remaining magic out of the world, so your sleigh can only hold \
                     so many presents at a time. When you run out, refill from a store. Those are \
                     close to where people live... right?",
                ))
                .wrap_to_pct(ctx, 50)
                .draw(ctx)
                .centered_horiz(),
                Widget::custom_row(level_buttons).flex_wrap(ctx, Percent::int(80)),
                "Created by Dustin Carlino, Yuwen Li, & Michael Kirk"
                    .draw_text(ctx)
                    .centered_horiz(),
                Btn::plaintext("Santa made by @parallaxcreativedesign")
                    .build(
                        ctx,
                        "open https://www.instagram.com/parallaxcreativedesign/",
                        None,
                    )
                    .centered_horiz(),
            ]))
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
                    return Transition::Pop;
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

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        app.session.music.draw(g);
    }
}

// TODO Preview the map, add padding, add the linear gradient...
fn locked_level(ctx: &mut EventCtx, app: &App, level: &Level, idx: usize) -> Widget {
    let mut txt = Text::new().bg(app.cs.unzoomed_bike);
    txt.add(Line(format!("LEVEL {}", idx + 1)).small_heading());
    txt.add(Line(&level.title).small_heading());
    txt.add(Line(&level.description));
    let mut batch = txt.wrap_to_pct(ctx, 15).render_to_batch(ctx.prerender);
    let hitbox = batch.get_bounds().get_rectangle();
    let center = hitbox.center();
    batch.push(app.cs.fade_map_dark, hitbox);
    batch.append(
        GeomBatch::load_svg(ctx.prerender, "system/assets/tools/locked.svg").centered_on(center),
    );
    Widget::draw_batch(ctx, batch)
}

fn unlocked_level(ctx: &mut EventCtx, app: &App, level: &Level, idx: usize) -> Widget {
    let mut txt = Text::new().bg(app.cs.unzoomed_bike);
    txt.add(Line(format!("LEVEL {}", idx + 1)).small_heading());
    txt.add(Line(&level.title).small_heading());
    txt.add(Line(&level.description));
    Btn::plaintext_custom(&level.title, txt.wrap_to_pct(ctx, 15)).build_def(ctx, None)
}
