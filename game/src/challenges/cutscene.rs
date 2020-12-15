use map_gui::tools::grey_out_map;
use widgetry::{
    hotkeys, Btn, Color, DrawBaselayer, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel,
    RewriteColor, State, Text, Widget,
};

use crate::app::App;
use crate::app::Transition;

pub struct CutsceneBuilder {
    name: String,
    scenes: Vec<Scene>,
}

enum Layout {
    PlayerSpeaking,
    BossSpeaking,
    Extra(&'static str, f64),
}

struct Scene {
    layout: Layout,
    msg: Text,
}

impl CutsceneBuilder {
    pub fn new(name: &str) -> CutsceneBuilder {
        CutsceneBuilder {
            name: name.to_string(),
            scenes: Vec::new(),
        }
    }

    pub fn player<I: Into<String>>(mut self, msg: I) -> CutsceneBuilder {
        self.scenes.push(Scene {
            layout: Layout::PlayerSpeaking,
            msg: Text::from(Line(msg).fg(Color::BLACK)),
        });
        self
    }

    pub fn boss<I: Into<String>>(mut self, msg: I) -> CutsceneBuilder {
        self.scenes.push(Scene {
            layout: Layout::BossSpeaking,
            msg: Text::from(Line(msg).fg(Color::BLACK)),
        });
        self
    }

    pub fn extra<I: Into<String>>(
        mut self,
        character: &'static str,
        scale: f64,
        msg: I,
    ) -> CutsceneBuilder {
        self.scenes.push(Scene {
            layout: Layout::Extra(character, scale),
            msg: Text::from(Line(msg).fg(Color::BLACK)),
        });
        self
    }

    pub fn build(
        self,
        ctx: &mut EventCtx,
        app: &App,
        make_task: Box<dyn Fn(&mut EventCtx) -> Widget>,
    ) -> Box<dyn State<App>> {
        Box::new(CutscenePlayer {
            panel: make_panel(ctx, app, &self.name, &self.scenes, &make_task, 0),
            name: self.name,
            scenes: self.scenes,
            idx: 0,
            make_task,
        })
    }
}

struct CutscenePlayer {
    name: String,
    scenes: Vec<Scene>,
    idx: usize,
    panel: Panel,
    make_task: Box<dyn Fn(&mut EventCtx) -> Widget>,
}

impl State<App> for CutscenePlayer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "quit" => {
                    // TODO Should SandboxMode use on_destroy for this?
                    app.primary.clear_sim();
                    app.set_prebaked(None);
                    return Transition::Multi(vec![Transition::Pop, Transition::Pop]);
                }
                "back" => {
                    self.idx -= 1;
                    self.panel = make_panel(
                        ctx,
                        app,
                        &self.name,
                        &self.scenes,
                        &self.make_task,
                        self.idx,
                    );
                }
                "next" => {
                    self.idx += 1;
                    self.panel = make_panel(
                        ctx,
                        app,
                        &self.name,
                        &self.scenes,
                        &self.make_task,
                        self.idx,
                    );
                }
                "Skip cutscene" => {
                    self.idx = self.scenes.len();
                    self.panel = make_panel(
                        ctx,
                        app,
                        &self.name,
                        &self.scenes,
                        &self.make_task,
                        self.idx,
                    );
                }
                "Start" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }
        // TODO Should the Panel for text widgets with wrapping do this instead?
        if ctx.input.is_window_resized() {
            self.panel = make_panel(
                ctx,
                app,
                &self.name,
                &self.scenes,
                &self.make_task,
                self.idx,
            );
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        // Happens to be a nice background color too ;)
        g.clear(app.cs.dialog_bg);
        self.panel.draw(g);
    }
}

fn make_panel(
    ctx: &mut EventCtx,
    app: &App,
    name: &str,
    scenes: &Vec<Scene>,
    make_task: &Box<dyn Fn(&mut EventCtx) -> Widget>,
    idx: usize,
) -> Panel {
    let prev = if idx > 0 {
        Btn::svg(
            "system/assets/tools/prev.svg",
            RewriteColor::Change(Color::WHITE, app.cs.hovering),
        )
        .build(ctx, "back", Key::LeftArrow)
    } else {
        Widget::draw_svg_transform(
            ctx,
            "system/assets/tools/prev.svg",
            RewriteColor::ChangeAlpha(0.3),
        )
    };
    let next = Btn::svg(
        "system/assets/tools/next.svg",
        RewriteColor::Change(Color::WHITE, app.cs.hovering),
    )
    .build(
        ctx,
        "next",
        hotkeys(vec![Key::RightArrow, Key::Space, Key::Enter]),
    );

    let inner = if idx == scenes.len() {
        Widget::custom_col(vec![
            (make_task)(ctx),
            Btn::txt("Start", Text::from(Line("Start").fg(Color::BLACK)))
                .build_def(ctx, Key::Enter)
                .centered_horiz()
                .align_bottom(),
        ])
    } else {
        Widget::custom_col(vec![
            match scenes[idx].layout {
                Layout::PlayerSpeaking => Widget::custom_row(vec![
                    Widget::draw_batch(
                        ctx,
                        GeomBatch::load_svg(ctx, "system/assets/characters/boss.svg")
                            .scale(0.75)
                            .autocrop(),
                    ),
                    Widget::custom_row(vec![
                        scenes[idx].msg.clone().wrap_to_pct(ctx, 30).draw(ctx),
                        Widget::draw_svg(ctx, "system/assets/characters/player.svg"),
                    ])
                    .align_right(),
                ]),
                Layout::BossSpeaking => Widget::custom_row(vec![
                    Widget::draw_batch(
                        ctx,
                        GeomBatch::load_svg(ctx, "system/assets/characters/boss.svg")
                            .scale(0.75)
                            .autocrop(),
                    ),
                    scenes[idx].msg.clone().wrap_to_pct(ctx, 30).draw(ctx),
                    Widget::draw_svg(ctx, "system/assets/characters/player.svg").align_right(),
                ]),
                Layout::Extra(name, scale) => Widget::custom_row(vec![
                    Widget::draw_batch(
                        ctx,
                        GeomBatch::load_svg(ctx, "system/assets/characters/boss.svg")
                            .scale(0.75)
                            .autocrop(),
                    ),
                    Widget::col(vec![
                        Widget::draw_batch(
                            ctx,
                            GeomBatch::load_svg(
                                ctx.prerender,
                                &format!("system/assets/characters/{}.svg", name),
                            )
                            .scale(scale)
                            .autocrop(),
                        ),
                        scenes[idx].msg.clone().wrap_to_pct(ctx, 30).draw(ctx),
                    ]),
                    Widget::draw_svg(ctx, "system/assets/characters/player.svg"),
                ])
                .evenly_spaced(),
            }
            .margin_above(100),
            Widget::col(vec![
                Widget::row(vec![prev, next]).centered_horiz(),
                Btn::txt(
                    "Skip cutscene",
                    Text::from(Line("Skip cutscene").fg(Color::BLACK)),
                )
                .build_def(ctx, None)
                .centered_horiz(),
            ])
            .align_bottom(),
        ])
    };

    let col = vec![
        // TODO Can't get this to alignment to work
        Widget::custom_row(vec![
            Btn::svg_def("system/assets/pregame/back.svg")
                .build(ctx, "quit", None)
                .margin_right(100),
            Line(name).big_heading_styled().draw(ctx),
        ])
        .margin_below(40),
        inner
            .fill_height()
            .padding(42)
            .bg(Color::WHITE)
            .outline(2.0, Color::BLACK),
    ];

    Panel::new(Widget::custom_col(col))
        .exact_size_percent(80, 80)
        .build_custom(ctx)
}

pub struct FYI {
    panel: Panel,
}

impl FYI {
    pub fn new(ctx: &mut EventCtx, contents: Widget, bg: Color) -> Box<dyn State<App>> {
        Box::new(FYI {
            panel: Panel::new(
                Widget::custom_col(vec![
                    contents,
                    Btn::txt("Okay", Text::from(Line("Okay").fg(Color::BLACK)))
                        .build_def(ctx, hotkeys(vec![Key::Escape, Key::Space, Key::Enter]))
                        .centered_horiz()
                        .align_bottom(),
                ])
                .padding(16)
                .bg(bg),
            )
            .exact_size_percent(50, 50)
            .build_custom(ctx),
        })
    }
}

impl State<App> for FYI {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Okay" => Transition::Pop,
                _ => unreachable!(),
            },
            _ => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}
