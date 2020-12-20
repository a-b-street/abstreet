use geom::Percent;
use map_gui::tools::open_browser;
use widgetry::{
    Btn, Color, EdgeInsets, EventCtx, GeomBatch, GfxCtx, Key, Line, Panel, RewriteColor,
    SimpleState, State, Text, TextExt, Widget,
};

use crate::levels::Level;
use crate::{App, Transition};

pub struct TitleScreen;

impl TitleScreen {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let mut level_buttons = Vec::new();
        for (idx, level) in app.session.levels.iter().enumerate() {
            if idx < app.session.levels_unlocked {
                level_buttons.push(unlocked_level(ctx, app, level, idx).margin_below(16));
            } else {
                level_buttons.push(locked_level(ctx, app, level, idx).margin_below(16));
            }
        }

        SimpleState::new(
            Panel::new(Widget::col(vec![
                Btn::svg_def("system/assets/tools/quit.svg")
                    .build(ctx, "quit", Key::Escape)
                    .align_right(),
                Line("15-minute Santa")
                    .display_title()
                    .draw(ctx)
                    .container()
                    .padding(16)
                    .bg(app.cs.fade_map_dark)
                    .centered_horiz(),
                Text::from(
                    Line(
                        "Time for Santa to deliver presents in Seattle! But... COVID means no \
                         stopping in houses to munch on cookies (gluten-free and paleo, \
                         obviously). When your blood sugar gets low, you'll have to stop and \
                         refill your supply from a store. Those are close to where people live... \
                         right?",
                    )
                    .small_heading(),
                )
                .wrap_to_pct(ctx, 50)
                .draw(ctx)
                .container()
                .padding(16)
                .bg(app.cs.fade_map_dark)
                .centered_horiz(),
                Widget::custom_row(level_buttons).flex_wrap(ctx, Percent::int(80)),
                Widget::row(vec![
                    Btn::text_bg1("Credits").build_def(ctx, None),
                    "Created by Dustin Carlino, Yuwen Li, & Michael Kirk"
                        .draw_text(ctx)
                        .container()
                        .padding(16)
                        .bg(app.cs.fade_map_dark),
                ])
                .centered_horiz()
                .align_bottom(),
            ]))
            .exact_size_percent(90, 85)
            .build_custom(ctx),
            Box::new(TitleScreen),
        )
    }
}

impl SimpleState<App> for TitleScreen {
    fn on_click(&mut self, ctx: &mut EventCtx, app: &mut App, x: &str, _: &Panel) -> Transition {
        match x {
            "quit" => Transition::Pop,
            "Credits" => Transition::Push(Credits::new(ctx)),
            x => {
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
        }
    }

    fn other_event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        app.session.update_music(ctx);
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        app.session.music.draw(g);
    }
}

fn level_btn(ctx: &mut EventCtx, app: &App, level: &Level, idx: usize) -> GeomBatch {
    let mut txt = Text::new();
    txt.add(Line(format!("LEVEL {}", idx + 1)).small_heading());
    txt.add(Line(&level.title).small_heading());
    txt.add(Line(&level.description));
    let batch = txt.wrap_to_pct(ctx, 15).render_autocropped(ctx);

    // Add padding
    let (mut batch, hitbox) = batch
        .batch()
        .container()
        .padding(EdgeInsets {
            top: 20.0,
            bottom: 20.0,
            left: 10.0,
            right: 10.0,
        })
        .to_geom(ctx, None);
    batch.unshift(app.cs.unzoomed_bike, hitbox);
    batch
}

// TODO Preview the map, add padding, add the linear gradient...
fn locked_level(ctx: &mut EventCtx, app: &App, level: &Level, idx: usize) -> Widget {
    let mut batch = level_btn(ctx, app, level, idx);
    let hitbox = batch.get_bounds().get_rectangle();
    let center = hitbox.center();
    batch.push(app.cs.fade_map_dark, hitbox);
    batch.append(GeomBatch::load_svg(ctx, "system/assets/tools/locked.svg").centered_on(center));
    Widget::draw_batch(ctx, batch)
}

fn unlocked_level(ctx: &mut EventCtx, app: &App, level: &Level, idx: usize) -> Widget {
    level_btn(ctx, app, level, idx)
        .to_btn_custom(RewriteColor::Change(
            Color::WHITE,
            ctx.style().hovering_color,
        ))
        .build(ctx, &level.title, None)
}

struct Credits;

impl Credits {
    fn new(ctx: &mut EventCtx) -> Box<dyn State<App>> {
        SimpleState::new(
            Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("15-minute Santa").big_heading_plain().draw(ctx),
                    Btn::close(ctx),
                ]),
                link(
                    ctx,
                    "Created by the A/B Street team",
                    "https://abstreet.org"
                ),
                Text::from_multiline(vec![
                    Line("Lead: Dustin Carlino"),
                    Line("Programming & game design: Michael Kirk"),
                    Line("UI/UX: Yuwen Li"),
                ]).draw(ctx),
                link(
                    ctx,
                    "Santa made by @parallaxcreativedesign",
                    "https://www.instagram.com/parallaxcreativedesign/",
                ),
                link(
                    ctx,
                    "Map data thanks to OpenStreetMap contributors",
                    "https://www.openstreetmap.org/about"),
                link(ctx, "Land use data from Seattle GeoData", "https://data-seattlecitygis.opendata.arcgis.com/datasets/current-land-use-zoning-detail"),
                link(ctx, "Music from various sources", "https://github.com/dabreegster/abstreet/tree/master/data/system/assets/music/sources.md"),
                link(ctx, "Fonts and icons by various sources", "https://dabreegster.github.io/abstreet/howto/#data-source-licensing"),
                "Playtesting by Fridgehaus".draw_text(ctx),
                Btn::text_bg2("Back").build_def(ctx, Key::Enter).centered_horiz(),
            ]))
            .build(ctx), Box::new(Credits))
    }
}

fn link(ctx: &mut EventCtx, label: &str, url: &str) -> Widget {
    Btn::plaintext(label).build(ctx, format!("open {}", url), None)
}

impl SimpleState<App> for Credits {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, x: &str, _: &Panel) -> Transition {
        match x {
            "close" | "Back" => Transition::Pop,
            x => {
                if let Some(url) = x.strip_prefix("open ") {
                    open_browser(url.to_string());
                    return Transition::Keep;
                }

                unreachable!()
            }
        }
    }

    fn other_event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        app.session.update_music(ctx);
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        app.session.music.draw(g);
    }
}
