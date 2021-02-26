use map_gui::tools::grey_out_map;
use widgetry::{
    hotkeys, ButtonStyle, Color, ControlState, DrawBaselayer, EventCtx, GeomBatch, GfxCtx, Image,
    Key, Line, Outcome, Panel, State, StyledButtons, Text, Widget,
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

    fn fg_color() -> Color {
        ButtonStyle::outline_dark_fg().fg
    }

    pub fn player<I: Into<String>>(mut self, msg: I) -> CutsceneBuilder {
        self.scenes.push(Scene {
            layout: Layout::PlayerSpeaking,
            msg: Text::from(Line(msg).fg(Self::fg_color())),
        });
        self
    }

    pub fn boss<I: Into<String>>(mut self, msg: I) -> CutsceneBuilder {
        self.scenes.push(Scene {
            layout: Layout::BossSpeaking,
            msg: Text::from(Line(msg).fg(Self::fg_color())),
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
            msg: Text::from(Line(msg).fg(Self::fg_color())),
        });
        self
    }

    pub fn build(
        self,
        ctx: &mut EventCtx,
        make_task: Box<dyn Fn(&mut EventCtx) -> Widget>,
    ) -> Box<dyn State<App>> {
        Box::new(CutscenePlayer {
            panel: make_panel(ctx, &self.name, &self.scenes, &make_task, 0),
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
                    self.panel =
                        make_panel(ctx, &self.name, &self.scenes, &self.make_task, self.idx);
                }
                "next" => {
                    self.idx += 1;
                    self.panel =
                        make_panel(ctx, &self.name, &self.scenes, &self.make_task, self.idx);
                }
                "Skip cutscene" => {
                    self.idx = self.scenes.len();
                    self.panel =
                        make_panel(ctx, &self.name, &self.scenes, &self.make_task, self.idx);
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
            self.panel = make_panel(ctx, &self.name, &self.scenes, &self.make_task, self.idx);
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
    name: &str,
    scenes: &Vec<Scene>,
    make_task: &Box<dyn Fn(&mut EventCtx) -> Widget>,
    idx: usize,
) -> Panel {
    let prev_builder = ButtonStyle::outline_dark_fg()
        .plain_icon("system/assets/tools/circled_prev.svg")
        .image_dims(45.0)
        .hotkey(Key::LeftArrow)
        .bg_color(Color::CLEAR, ControlState::Disabled);

    let next = prev_builder
        .clone()
        .image_path("system/assets/tools/circled_next.svg")
        .hotkey(hotkeys(vec![Key::RightArrow, Key::Space, Key::Enter]))
        .build_widget(ctx, "next");

    let prev = prev_builder.disabled(idx == 0).build_widget(ctx, "back");

    let inner = if idx == scenes.len() {
        Widget::custom_col(vec![
            (make_task)(ctx),
            ctx.style()
                .btn_solid_primary_text("Start")
                .hotkey(Key::Enter)
                .build_def(ctx)
                .centered_horiz()
                .align_bottom(),
        ])
    } else {
        Widget::custom_col(vec![
            match scenes[idx].layout {
                Layout::PlayerSpeaking => Widget::custom_row(vec![
                    Widget::draw_batch(
                        ctx,
                        GeomBatch::load_svg(ctx, "system/assets/characters/boss.svg.gz")
                            .scale(0.75)
                            .autocrop(),
                    ),
                    Widget::custom_row(vec![
                        scenes[idx].msg.clone().wrap_to_pct(ctx, 30).draw(ctx),
                        Image::untinted("system/assets/characters/player.svg").into_widget(ctx),
                    ])
                    .align_right(),
                ]),
                Layout::BossSpeaking => Widget::custom_row(vec![
                    Widget::draw_batch(
                        ctx,
                        GeomBatch::load_svg(ctx, "system/assets/characters/boss.svg.gz")
                            .scale(0.75)
                            .autocrop(),
                    ),
                    scenes[idx].msg.clone().wrap_to_pct(ctx, 30).draw(ctx),
                    Image::untinted("system/assets/characters/player.svg")
                        .into_widget(ctx)
                        .align_right(),
                ]),
                Layout::Extra(filename, scale) => Widget::custom_row(vec![
                    Widget::draw_batch(
                        ctx,
                        GeomBatch::load_svg(ctx, "system/assets/characters/boss.svg.gz")
                            .scale(0.75)
                            .autocrop(),
                    ),
                    Widget::col(vec![
                        Widget::draw_batch(
                            ctx,
                            GeomBatch::load_svg(
                                ctx.prerender,
                                &format!("system/assets/characters/{}", filename),
                            )
                            .scale(scale)
                            .autocrop(),
                        ),
                        scenes[idx].msg.clone().wrap_to_pct(ctx, 30).draw(ctx),
                    ]),
                    Image::untinted("system/assets/characters/player.svg").into_widget(ctx),
                ])
                .evenly_spaced(),
            }
            .margin_above(100),
            Widget::col(vec![
                Widget::row(vec![prev.margin_right(40), next]).centered_horiz(),
                ButtonStyle::outline_dark_fg()
                    .outline_text("Skip cutscene")
                    .build_def(ctx)
                    .centered_horiz(),
            ])
            .align_bottom(),
        ])
    };

    let col = vec![
        // TODO Can't get this to alignment to work
        Widget::custom_row(vec![
            ctx.style()
                .btn_back("Home")
                .build_widget(ctx, "quit")
                .margin_right(100),
            Line(name).big_heading_styled().draw(ctx),
        ])
        .margin_below(40),
        inner
            .fill_height()
            .padding(42)
            .bg(Color::WHITE)
            .outline(2.0, CutsceneBuilder::fg_color()),
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
                    ctx.style()
                        .btn_solid_primary_text("OK")
                        .hotkey(hotkeys(vec![Key::Escape, Key::Space, Key::Enter]))
                        .build_def(ctx)
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
                "OK" => Transition::Pop,
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
