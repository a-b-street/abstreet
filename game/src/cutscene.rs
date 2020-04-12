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

    pub fn scene(mut self, avatar: &str, msg: &str) -> CutsceneBuilder {
        self.scenes.push(Scene {
            avatar: Some(avatar.to_string()),
            msg: Text::from(Line(msg)),
        });
        self
    }

    pub fn narrator(mut self, msg: &str) -> CutsceneBuilder {
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
                "Skip" | "Start" => {
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
        .margin(5)
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

    let mut col = vec![];
    let msg = scenes[idx].msg.clone();
    if let Some(ref name) = scenes[idx].avatar {
        col.push(Widget::row(vec![
            Widget::draw_svg(
                ctx,
                format!("../data/system/assets/characters/{}.svg", name),
            ),
            msg.draw(ctx),
        ]));
    } else {
        col.push(msg.draw(ctx));
    }

    col.push(Widget::row(vec![prev, next]));
    if idx == scenes.len() - 1 {
        col.push(
            Btn::text_bg2("Start")
                .build_def(ctx, hotkeys(vec![Key::RightArrow, Key::Space, Key::Enter])),
        );
    } else {
        col.push(Btn::text_bg2("Skip").build_def(ctx, None));
    }

    Composite::new(Widget::col(col)).build(ctx)
}
