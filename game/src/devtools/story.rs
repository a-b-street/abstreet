use crate::app::App;
use crate::game::{State, Transition};
use ezgui::{
    hotkey, lctrl, Btn, Color, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, VerticalAlignment, Widget,
};
use geom::{LonLat, Polygon};
use serde::{Deserialize, Serialize};

pub struct StoryMapEditor {
    composite: Composite,
    story: StoryMap,
}

impl StoryMapEditor {
    pub fn new(ctx: &mut EventCtx, app: &App, story: StoryMap) -> Box<dyn State> {
        Box::new(StoryMapEditor {
            composite: Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        Line("Story map editor")
                            .small_heading()
                            .draw(ctx)
                            .margin_right(5),
                        Widget::draw_batch(
                            ctx,
                            GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 30.0))]),
                        )
                        .margin_right(5),
                        Btn::text_fg(format!("{} ▼", story.name))
                            .build(ctx, "load story map", lctrl(Key::L))
                            .margin_right(5),
                        Btn::svg_def("../data/system/assets/tools/save.svg")
                            .build(ctx, "save", lctrl(Key::S))
                            .margin_right(5),
                        Btn::plaintext("X")
                            .build(ctx, "close", hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    Widget::row(vec![Btn::svg_def(
                        "../data/system/assets/timeline/goal_pos.svg",
                    )
                    .build(ctx, "new marker", None)]),
                ])
                .padding(16)
                .bg(app.cs.panel_bg),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            story,
        })
    }
}

impl State for StoryMapEditor {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        ctx.canvas_movement();

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.composite.draw(g);
    }
}

#[derive(Serialize, Deserialize)]
pub struct StoryMap {
    name: String,
    markers: Vec<(LonLat, String)>,
}

impl StoryMap {
    pub fn empty() -> StoryMap {
        StoryMap {
            name: "new story".to_string(),
            markers: Vec::new(),
        }
    }
}
