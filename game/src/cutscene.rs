use crate::app::App;
use crate::game::{DrawBaselayer, State, Transition};
use ezgui::{
    hotkey, hotkeys, Btn, Color, Composite, EventCtx, GfxCtx, Key, Line, Outcome, RewriteColor,
    Text, Widget,
};

pub struct CutsceneBuilder {
    name: String,
    scenes: Vec<Scene>,
}

struct Scene {
    // TODO else boss. Simple till we have a 3-person scene.
    player: bool,
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
            player: true,
            msg: Text::from(Line(msg).fg(Color::BLACK)),
        });
        self
    }

    pub fn boss<I: Into<String>>(mut self, msg: I) -> CutsceneBuilder {
        self.scenes.push(Scene {
            player: false,
            msg: Text::from(Line(msg).fg(Color::BLACK)),
        });
        self
    }

    // TODO Remove
    pub fn narrator<I: Into<String>>(mut self, msg: I) -> CutsceneBuilder {
        self.scenes.push(Scene {
            player: true,
            msg: Text::from(Line(msg).fg(Color::BLACK)),
        });
        self
    }

    pub fn build(
        self,
        ctx: &mut EventCtx,
        app: &App,
        make_task: fn(&mut EventCtx) -> Widget,
    ) -> Box<dyn State> {
        Box::new(CutscenePlayer {
            composite: make_panel(ctx, app, &self.name, &self.scenes, &make_task, 0),
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
    composite: Composite,
    make_task: fn(&mut EventCtx) -> Widget,
}

impl State for CutscenePlayer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "quit" => {
                    return Transition::Pop;
                }
                "back" => {
                    self.idx -= 1;
                    self.composite = make_panel(
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
                    self.composite = make_panel(
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
                    self.composite = make_panel(
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
            None => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        // Happens to be a nice background color too ;)
        g.clear(app.cs.grass);
        self.composite.draw(g);
    }
}

fn make_panel(
    ctx: &mut EventCtx,
    app: &App,
    name: &str,
    scenes: &Vec<Scene>,
    make_task: &fn(&mut EventCtx) -> Widget,
    idx: usize,
) -> Composite {
    let prev = if idx > 0 {
        Btn::svg(
            "../data/system/assets/tools/prev.svg",
            RewriteColor::Change(Color::WHITE, app.cs.hovering),
        )
        .build(ctx, "back", hotkey(Key::LeftArrow))
    } else {
        Widget::draw_svg_transform(
            ctx,
            "../data/system/assets/tools/prev.svg",
            RewriteColor::ChangeAlpha(0.3),
        )
    };
    let next = Btn::svg(
        "../data/system/assets/tools/next.svg",
        RewriteColor::Change(Color::WHITE, app.cs.hovering),
    )
    .build(
        ctx,
        "next",
        hotkeys(vec![Key::RightArrow, Key::Space, Key::Enter]),
    );

    let inner = if idx == scenes.len() {
        Widget::col(vec![
            (make_task)(ctx),
            Btn::text_fg_line("Start", Line("Start").fg(Color::BLACK))
                .build_def(ctx, hotkey(Key::Enter))
                .centered_horiz()
                .align_bottom(),
        ])
    } else {
        Widget::col(vec![
            // TODO Can't get this centered better
            if scenes[idx].player {
                Widget::row(vec![
                    Widget::draw_svg(ctx, "../data/system/assets/characters/boss.svg"),
                    Widget::row(vec![
                        scenes[idx].msg.clone().draw(ctx),
                        Widget::draw_svg(ctx, "../data/system/assets/characters/player.svg"),
                    ])
                    .align_right(),
                ])
            } else {
                Widget::row(vec![
                    Widget::draw_svg(ctx, "../data/system/assets/characters/boss.svg"),
                    scenes[idx].msg.clone().draw(ctx),
                    Widget::draw_svg(ctx, "../data/system/assets/characters/player.svg")
                        .align_right(),
                ])
            }
            .margin_above(100),
            Widget::col(vec![
                Widget::row(vec![prev.margin_right(15), next])
                    .centered_horiz()
                    .margin_below(10),
                Btn::text_fg_line("Skip cutscene", Line("Skip cutscene").fg(Color::BLACK))
                    .build_def(ctx, None)
                    .centered_horiz(),
            ])
            .align_bottom(),
        ])
    };

    let col = vec![
        // TODO Can't get this to alignment to work
        Widget::row(vec![
            Btn::svg_def("../data/system/assets/pregame/back.svg")
                .build(ctx, "quit", None)
                .margin_right(100),
            Line(name).big_heading_styled().draw(ctx),
        ])
        .margin_below(50),
        inner
            .fill_height()
            .padding(42)
            .bg(Color::WHITE)
            .outline(2.0, Color::BLACK),
    ];

    Composite::new(Widget::col(col))
        .exact_size_percent(80, 80)
        .build(ctx)
}

pub struct FYI {
    composite: Composite,
}

impl FYI {
    pub fn new(ctx: &mut EventCtx, contents: Widget, bg: Color) -> Box<dyn State> {
        Box::new(FYI {
            composite: Composite::new(
                Widget::col(vec![
                    contents,
                    Btn::text_fg_line("Okay", Line("Okay").fg(Color::BLACK))
                        .build_def(ctx, hotkeys(vec![Key::Escape, Key::Space, Key::Enter]))
                        .centered_horiz()
                        .align_bottom(),
                ])
                .padding(16)
                .bg(bg),
            )
            .build(ctx),
        })
    }
}

impl State for FYI {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Okay" => Transition::Pop,
                _ => unreachable!(),
            },
            None => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.composite.draw(g);
    }
}
