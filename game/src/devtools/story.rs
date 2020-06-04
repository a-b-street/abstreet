use crate::app::{App, ShowEverything};
use crate::common::CommonState;
use crate::game::{DrawBaselayer, State, Transition, WizardState};
use crate::render::DrawOptions;
use ezgui::{
    hotkey, lctrl, Btn, Choice, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, RewriteColor, Text, VerticalAlignment, Widget,
};
use geom::{Angle, LonLat, Polygon, Pt2D};
use serde::{Deserialize, Serialize};

// TODO This is a really great example of things that ezgui ought to make easier. Maybe a radio
// button-ish thing to start?

// Good inspiration: http://sfo-assess.dha.io/, https://github.com/mapbox/storytelling,
// https://storymap.knightlab.com/

pub struct StoryMapEditor {
    composite: Composite,
    story: StoryMap,
    mode: Mode,
    dirty: bool,

    // Index into story.markers
    // TODO Stick in Mode::View?
    hovering: Option<usize>,
}

enum Mode {
    View,
    Placing,
    Dragging(usize),
    Editing(usize, Composite),
}

impl StoryMapEditor {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        let story = StoryMap::new();
        let mode = Mode::View;
        let dirty = false;
        Box::new(StoryMapEditor {
            composite: make_panel(ctx, app, &story, &mode, dirty),
            story,
            mode,
            dirty,
            hovering: None,
        })
    }

    fn redo_panel(&mut self, ctx: &mut EventCtx, app: &App) {
        self.composite = make_panel(ctx, app, &self.story, &self.mode, self.dirty);
    }
}

impl State for StoryMapEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.mode {
            Mode::View => {
                ctx.canvas_movement();

                if ctx.redo_mouseover() {
                    self.hovering = None;
                    if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                        self.hovering = self
                            .story
                            .markers
                            .iter()
                            .position(|m| m.hitbox.contains_pt(pt));
                    }
                }
                if let Some(idx) = self.hovering {
                    if ctx
                        .input
                        .key_pressed(Key::LeftControl, "hold to move this marker")
                    {
                        self.mode = Mode::Dragging(idx);
                    } else if app.per_obj.left_click(ctx, "edit marker") {
                        self.mode =
                            Mode::Editing(idx, self.story.markers[idx].make_editor(ctx, app));
                    }
                }
            }
            Mode::Placing => {
                ctx.canvas_movement();

                if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                    if app.primary.map.get_boundary_polygon().contains_pt(pt)
                        && app.per_obj.left_click(ctx, "place a marker here")
                    {
                        let idx = self.story.markers.len();
                        self.story.markers.push(Marker::new(ctx, pt, String::new()));
                        self.dirty = true;
                        self.redo_panel(ctx, app);
                        self.mode =
                            Mode::Editing(idx, self.story.markers[idx].make_editor(ctx, app));
                    }
                }
            }
            Mode::Dragging(idx) => {
                if ctx.redo_mouseover() {
                    if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                        if app.primary.map.get_boundary_polygon().contains_pt(pt) {
                            self.story.markers[idx] =
                                Marker::new(ctx, pt, self.story.markers[idx].event.clone());
                            self.dirty = true;
                            self.redo_panel(ctx, app);
                        }
                    }
                }

                if ctx.input.key_released(Key::LeftControl) {
                    self.mode = Mode::View;
                }
            }
            Mode::Editing(idx, ref mut composite) => {
                ctx.canvas_movement();
                match composite.event(ctx) {
                    Some(Outcome::Clicked(x)) => match x.as_ref() {
                        "close" => {
                            self.mode = Mode::View;
                            self.redo_panel(ctx, app);
                        }
                        "confirm" => {
                            self.story.markers[idx] = Marker::new(
                                ctx,
                                self.story.markers[idx].pt,
                                composite.text_box("event"),
                            );
                            self.dirty = true;
                            self.mode = Mode::View;
                            self.redo_panel(ctx, app);
                        }
                        "delete" => {
                            self.mode = Mode::View;
                            self.hovering = None;
                            self.story.markers.remove(idx);
                            self.dirty = true;
                            self.redo_panel(ctx, app);
                        }
                        _ => unreachable!(),
                    },
                    None => {}
                }
            }
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    // TODO autosave
                    return Transition::Pop;
                }
                "save" => {
                    if self.story.name == "new story" {
                        return Transition::Push(WizardState::new(Box::new(|wiz, ctx, _| {
                            let name = wiz.wrap(ctx).input_string("Name this story map")?;
                            Some(Transition::PopWithData(Box::new(move |state, ctx, app| {
                                let editor = state.downcast_mut::<StoryMapEditor>().unwrap();
                                editor.story.name = name;
                                editor.story.save(app);
                                editor.dirty = false;
                                editor.redo_panel(ctx, app);
                            })))
                        })));
                    } else {
                        self.story.save(app);
                        self.dirty = false;
                        self.redo_panel(ctx, app);
                    }
                }
                "load" => {
                    // TODO autosave
                    let current = self.story.name.clone();
                    let btn = self.composite.rect_of("load").clone();
                    return Transition::Push(WizardState::new(Box::new(move |wiz, ctx, app| {
                        let (_, raw) = wiz.wrap(ctx).choose_exact(
                            (
                                HorizontalAlignment::Centered(btn.center().x),
                                VerticalAlignment::Below(btn.y2 + 15.0),
                            ),
                            None,
                            || {
                                let mut list = Vec::new();
                                for (name, story) in abstutil::load_all_objects::<RecordedStoryMap>(
                                    "../data/player/stories".to_string(),
                                ) {
                                    if story.name == current {
                                        continue;
                                    }
                                    // TODO Argh, we can't make StoryMap cloneable, so redo some
                                    // work
                                    if app
                                        .primary
                                        .map
                                        .get_gps_bounds()
                                        .try_convert(
                                            &story.markers.iter().map(|(gps, _)| *gps).collect(),
                                        )
                                        .is_some()
                                    {
                                        list.push(Choice::new(name, story));
                                    }
                                }
                                list.push(Choice::new(
                                    "new story",
                                    RecordedStoryMap {
                                        name: "new story".to_string(),
                                        markers: Vec::new(),
                                    },
                                ));
                                list
                            },
                        )?;
                        let story = StoryMap::load(ctx, app, raw).unwrap();
                        Some(Transition::PopWithData(Box::new(move |state, ctx, app| {
                            let editor = state.downcast_mut::<StoryMapEditor>().unwrap();
                            editor.story = story;
                            editor.dirty = false;
                            editor.redo_panel(ctx, app);
                        })))
                    })));
                }
                "new marker" => {
                    self.hovering = None;
                    self.mode = Mode::Placing;
                    self.redo_panel(ctx, app);
                }
                "pan" => {
                    self.mode = Mode::View;
                    self.redo_panel(ctx, app);
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
        let mut opts = DrawOptions::new();
        opts.label_buildings = true;
        app.draw(g, opts, &app.primary.sim, &ShowEverything::new());

        match self.mode {
            Mode::Placing => {
                if g.canvas.get_cursor_in_map_space().is_some() {
                    let mut batch = GeomBatch::new();
                    batch.add_svg(
                        g.prerender,
                        "../data/system/assets/timeline/goal_pos.svg",
                        g.canvas.get_cursor().to_pt(),
                        1.0,
                        Angle::ZERO,
                        RewriteColor::Change(Color::hex("#5B5B5B"), Color::GREEN),
                        false,
                    );
                    g.fork_screenspace();
                    batch.draw(g);
                    g.unfork();
                }
            }
            Mode::Editing(_, ref composite) => {
                composite.draw(g);
            }
            _ => {}
        }

        for (idx, m) in self.story.markers.iter().enumerate() {
            if self.hovering == Some(idx) {
                m.draw_hovered(g, app);
            } else {
                g.redraw(&m.draw);
            }
        }

        self.composite.draw(g);
        CommonState::draw_osd(g, app, &None);
    }
}

fn make_panel(
    ctx: &mut EventCtx,
    app: &App,
    story: &StoryMap,
    mode: &Mode,
    dirty: bool,
) -> Composite {
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
                    .build(ctx, "load", lctrl(Key::L))
                    .margin_right(5),
                if dirty {
                    Btn::svg_def("../data/system/assets/tools/save.svg").build(
                        ctx,
                        "save",
                        lctrl(Key::S),
                    )
                } else {
                    Widget::draw_svg_transform(
                        ctx,
                        "../data/system/assets/tools/save.svg",
                        RewriteColor::ChangeAlpha(0.5),
                    )
                }
                .margin_right(5),
                Btn::plaintext("X")
                    .build(ctx, "close", hotkey(Key::Escape))
                    .align_right(),
            ]),
            Widget::row(vec![
                if let Mode::Placing = mode {
                    Widget::draw_svg_transform(
                        ctx,
                        "../data/system/assets/timeline/goal_pos.svg",
                        RewriteColor::Change(Color::hex("#5B5B5B"), Color::hex("#4CA7E9")),
                    )
                } else {
                    Btn::svg_def("../data/system/assets/timeline/goal_pos.svg").build(
                        ctx,
                        "new marker",
                        hotkey(Key::M),
                    )
                },
                if let Mode::View = mode {
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

#[derive(Clone, Serialize, Deserialize)]
struct RecordedStoryMap {
    name: String,
    markers: Vec<(LonLat, String)>,
}
impl abstutil::Cloneable for RecordedStoryMap {}

struct StoryMap {
    name: String,
    markers: Vec<Marker>,
}

struct Marker {
    pt: Pt2D,
    event: String,
    hitbox: Polygon,
    draw: Drawable,
}

impl StoryMap {
    fn new() -> StoryMap {
        StoryMap {
            name: "new story".to_string(),
            markers: Vec::new(),
        }
    }

    fn load(ctx: &mut EventCtx, app: &App, story: RecordedStoryMap) -> Option<StoryMap> {
        let mut markers = Vec::new();
        for (gps, event) in story.markers {
            let pt = Pt2D::from_gps(gps, app.primary.map.get_gps_bounds())?;
            markers.push(Marker::new(ctx, pt, event));
        }
        Some(StoryMap {
            name: story.name,
            markers,
        })
    }

    fn save(&self, app: &App) {
        let story = RecordedStoryMap {
            name: self.name.clone(),
            markers: self
                .markers
                .iter()
                .map(|m| {
                    (
                        m.pt.to_gps(app.primary.map.get_gps_bounds()).unwrap(),
                        m.event.clone(),
                    )
                })
                .collect(),
        };
        abstutil::write_json(
            format!("../data/player/stories/{}.json", story.name),
            &story,
        );
    }
}

impl Marker {
    fn new(ctx: &mut EventCtx, pt: Pt2D, event: String) -> Marker {
        let mut batch = GeomBatch::new();
        batch.add_svg(
            ctx.prerender,
            "../data/system/assets/timeline/goal_pos.svg",
            pt,
            2.0,
            Angle::ZERO,
            RewriteColor::Change(Color::hex("#5B5B5B"), Color::hex("#FE3D00")),
            false,
        );
        batch.add_transformed(
            Text::from(Line(&event))
                .with_bg()
                .render_to_batch(ctx.prerender),
            pt,
            0.5,
            Angle::ZERO,
            RewriteColor::NoOp,
        );
        Marker {
            pt,
            event,
            hitbox: batch.unioned_polygon(),
            draw: ctx.upload(batch),
        }
    }

    fn draw_hovered(&self, g: &mut GfxCtx, app: &App) {
        let mut batch = GeomBatch::new();
        batch.add_svg(
            g.prerender,
            "../data/system/assets/timeline/goal_pos.svg",
            self.pt,
            2.0,
            Angle::ZERO,
            RewriteColor::Change(Color::hex("#5B5B5B"), app.cs.hovering),
            false,
        );
        batch.add_transformed(
            Text::from(Line(&self.event))
                .with_bg()
                .render_to_batch(g.prerender),
            self.pt,
            0.75,
            Angle::ZERO,
            RewriteColor::NoOp,
        );
        batch.draw(g);
    }

    fn make_editor(&self, ctx: &mut EventCtx, app: &App) -> Composite {
        Composite::new(
            Widget::col(vec![
                Widget::row(vec![
                    Line("Editing marker").small_heading().draw(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                Btn::text_fg("delete").build_def(ctx, None),
                Widget::text_entry(ctx, self.event.clone(), true).named("event"),
                Btn::text_fg("confirm").build_def(ctx, hotkey(Key::Enter)),
            ])
            .padding(16)
            .bg(app.cs.panel_bg),
        )
        .build(ctx)
    }
}
