use crate::app::App;
use crate::common::CommonState;
use crate::game::{State, Transition, WizardState};
use ezgui::{
    hotkey, lctrl, Btn, Color, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, RewriteColor, Text, VerticalAlignment, Widget,
};
use geom::{Angle, LonLat, Polygon, Pt2D};
use serde::{Deserialize, Serialize};

// Good inspiration: http://sfo-assess.dha.io/

pub struct StoryMapEditor {
    composite: Composite,
    story: StoryMap,
    mode: Mode,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    View,
    Marker,
}

impl StoryMapEditor {
    pub fn new(ctx: &mut EventCtx, app: &App, story: StoryMap) -> Box<dyn State> {
        let mode = Mode::View;
        Box::new(StoryMapEditor {
            composite: make_panel(ctx, app, &story, mode),
            story,
            mode,
        })
    }
}

impl State for StoryMapEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.mode {
            Mode::View => {}
            Mode::Marker => {
                if let Some(gps) = ctx
                    .canvas
                    .get_cursor_in_map_space()
                    .and_then(|pt| pt.to_gps(app.primary.map.get_gps_bounds()))
                {
                    if app.per_obj.left_click(ctx, "place a marker here") {
                        self.mode = Mode::View;
                        self.composite = make_panel(ctx, app, &self.story, self.mode);

                        return Transition::Push(WizardState::new(Box::new(move |wiz, ctx, _| {
                            let event = wiz.wrap(ctx).input_string("What happened here?")?;
                            Some(Transition::PopWithData(Box::new(move |state, _, _| {
                                let editor = state.downcast_mut::<StoryMapEditor>().unwrap();
                                editor.story.markers.push((gps, event));
                            })))
                        })));
                    }
                }
            }
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "new marker" => {
                    self.mode = Mode::Marker;
                    self.composite = make_panel(ctx, app, &self.story, self.mode);
                }
                "pan" => {
                    self.mode = Mode::View;
                    self.composite = make_panel(ctx, app, &self.story, self.mode);
                }
                _ => unreachable!(),
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        if self.mode == Mode::Marker && g.canvas.get_cursor_in_map_space().is_some() {
            let mut batch = GeomBatch::new();
            batch.add_svg(
                g.prerender,
                "../data/system/assets/timeline/goal_pos.svg",
                g.canvas.get_cursor().to_pt(),
                1.0,
                Angle::ZERO,
                RewriteColor::ChangeAll(Color::GREEN),
                false,
            );
            g.fork_screenspace();
            batch.draw(g);
            g.unfork();
        }

        self.story.render(g, app).draw(g);

        self.composite.draw(g);
        CommonState::draw_osd(g, app, &None);
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

    fn render(&self, g: &mut GfxCtx, app: &App) -> GeomBatch {
        let mut batch = GeomBatch::new();
        for (gps, event) in &self.markers {
            let pt = Pt2D::from_gps(*gps, app.primary.map.get_gps_bounds()).unwrap();
            batch.add_svg(
                g.prerender,
                "../data/system/assets/timeline/goal_pos.svg",
                pt,
                1.0,
                Angle::ZERO,
                RewriteColor::NoOp,
                false,
            );
            batch.add_transformed(
                Text::from(Line(event))
                    .with_bg()
                    .render_to_batch(g.prerender),
                pt,
                0.1,
                Angle::ZERO,
                RewriteColor::NoOp,
            );
        }
        batch
    }
}

fn make_panel(ctx: &mut EventCtx, app: &App, story: &StoryMap, mode: Mode) -> Composite {
    Composite::new(
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
            Widget::row(vec![
                if mode == Mode::Marker {
                    Widget::draw_svg_transform(
                        ctx,
                        "../data/system/assets/timeline/goal_pos.svg",
                        RewriteColor::ChangeAll(Color::hex("#4CA7E9")),
                    )
                } else {
                    Btn::svg_def("../data/system/assets/timeline/goal_pos.svg").build(
                        ctx,
                        "new marker",
                        hotkey(Key::M),
                    )
                },
                if mode == Mode::View {
                    Widget::draw_svg_transform(
                        ctx,
                        "../data/system/assets/tools/pan.svg",
                        RewriteColor::ChangeAll(Color::hex("#4CA7E9")),
                    )
                } else {
                    Btn::svg_def("../data/system/assets/tools/pan.svg").build(
                        ctx,
                        "pan",
                        hotkey(Key::Escape),
                    )
                },
            ])
            .evenly_spaced(),
        ])
        .padding(16)
        .bg(app.cs.panel_bg),
    )
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}
