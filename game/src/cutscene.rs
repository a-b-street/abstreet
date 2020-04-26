use crate::app::App;
use crate::game::{State, Transition};
use ezgui::{
    hotkey, hotkeys, Btn, Color, Composite, EventCtx, GfxCtx, Key, Line, Outcome, RewriteColor,
    Text, Widget,
};

pub struct CutsceneBuilder {
    scenes: Vec<Scene>,
}

struct Scene {
    avatar: Option<String>,
    msg: Text,
}

impl CutsceneBuilder {
    pub fn new() -> CutsceneBuilder {
        CutsceneBuilder { scenes: Vec::new() }
    }

    pub fn scene<I: Into<String>>(mut self, avatar: &str, msg: I) -> CutsceneBuilder {
        self.scenes.push(Scene {
            avatar: Some(avatar.to_string()),
            msg: Text::from(Line(msg)),
        });
        self
    }

    pub fn narrator<I: Into<String>>(mut self, msg: I) -> CutsceneBuilder {
        self.scenes.push(Scene {
            avatar: None,
            msg: Text::from(Line(msg)),
        });
        self
    }

    pub fn build(self, ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        Box::new(CutscenePlayer {
            composite: make_panel(ctx, app, &self.scenes, 0),
            scenes: self.scenes,
            idx: 0,
        })
    }
}

struct CutscenePlayer {
    scenes: Vec<Scene>,
    idx: usize,
    composite: Composite,
}

impl State for CutscenePlayer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "back" => {
                    self.idx -= 1;
                    self.composite = make_panel(ctx, app, &self.scenes, self.idx);
                }
                "next" => {
                    self.idx += 1;
                    self.composite = make_panel(ctx, app, &self.scenes, self.idx);
                }
                "Skip cutscene" | "Start" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        // Happens to be a nice background color too ;)
        g.clear(app.cs.grass);
        self.composite.draw(g);
    }
}

fn make_panel(ctx: &mut EventCtx, app: &App, scenes: &Vec<Scene>, idx: usize) -> Composite {
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
            RewriteColor::ChangeAll(Color::WHITE.alpha(0.5)),
        )
    };
    let next = if idx == scenes.len() - 1 {
        Widget::draw_svg_transform(
            ctx,
            "../data/system/assets/tools/next.svg",
            RewriteColor::ChangeAll(Color::WHITE.alpha(0.5)),
        )
    } else {
        Btn::svg(
            "../data/system/assets/tools/next.svg",
            RewriteColor::Change(Color::WHITE, app.cs.hovering),
        )
        .build(
            ctx,
            "next",
            hotkeys(vec![Key::RightArrow, Key::Space, Key::Enter]),
        )
    };

    let mut col = vec![Widget::row(vec![
        if let Some(ref name) = scenes[idx].avatar {
            Widget::draw_svg(
                ctx,
                format!("../data/system/assets/characters/{}.svg", name),
            )
            .container()
            .outline(10.0, Color::BLACK)
            .padding(10)
        } else {
            Widget::nothing()
        },
        scenes[idx]
            .msg
            .clone()
            .draw(ctx)
            .container()
            .outline(10.0, Color::BLACK)
            .padding(10),
    ])
    .bg(app.cs.panel_bg)
    .margin_below(10)];

    col.push(
        Widget::row(vec![prev.margin_right(15), next])
            .centered_horiz()
            .margin_below(10),
    );

    col.push(
        (if idx == scenes.len() - 1 {
            Btn::text_bg2("Start")
                .build_def(ctx, hotkeys(vec![Key::RightArrow, Key::Space, Key::Enter]))
        } else {
            Btn::text_bg2("Skip cutscene").build_def(ctx, None)
        })
        .centered_horiz(),
    );

    Composite::new(Widget::col(col)).build(ctx)
}
