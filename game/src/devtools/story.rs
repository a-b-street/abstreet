use serde::{Deserialize, Serialize};

use geom::{Distance, LonLat, PolyLine, Polygon, Pt2D, Ring};
use map_gui::render::DrawOptions;
use map_gui::tools::{ChooseSomething, PromptInput};
use widgetry::{
    lctrl, Btn, Choice, Color, DrawBaselayer, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, Panel, RewriteColor, State, Text, VerticalAlignment,
    Widget,
};

use crate::app::{App, ShowEverything, Transition};
use crate::common::CommonState;

// TODO This is a really great example of things that widgetry ought to make easier. Maybe a radio
// button-ish thing to start?

// Good inspiration: http://sfo-assess.dha.io/, https://github.com/mapbox/storytelling,
// https://storymap.knightlab.com/

pub struct StoryMapEditor {
    panel: Panel,
    story: StoryMap,
    mode: Mode,
    dirty: bool,

    // Index into story.markers
    // TODO Stick in Mode::View?
    hovering: Option<usize>,
}

enum Mode {
    View,
    PlacingMarker,
    Dragging(Pt2D, usize),
    Editing(usize, Panel),
    Freehand(Option<Lasso>),
}

impl StoryMapEditor {
    pub fn new(ctx: &mut EventCtx) -> Box<dyn State<App>> {
        let story = StoryMap::new();
        let mode = Mode::View;
        let dirty = false;
        Box::new(StoryMapEditor {
            panel: make_panel(ctx, &story, &mode, dirty),
            story,
            mode,
            dirty,
            hovering: None,
        })
    }

    fn redo_panel(&mut self, ctx: &mut EventCtx) {
        self.panel = make_panel(ctx, &self.story, &self.mode, self.dirty);
    }
}

impl State<App> for StoryMapEditor {
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
                    if ctx.input.pressed(Key::LeftControl) {
                        self.mode =
                            Mode::Dragging(ctx.canvas.get_cursor_in_map_space().unwrap(), idx);
                    } else if app.per_obj.left_click(ctx, "edit marker") {
                        self.mode = Mode::Editing(idx, self.story.markers[idx].make_editor(ctx));
                    }
                }
            }
            Mode::PlacingMarker => {
                ctx.canvas_movement();

                if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                    if app.primary.map.get_boundary_polygon().contains_pt(pt)
                        && app.per_obj.left_click(ctx, "place a marker here")
                    {
                        let idx = self.story.markers.len();
                        self.story
                            .markers
                            .push(Marker::new(ctx, vec![pt], String::new()));
                        self.dirty = true;
                        self.redo_panel(ctx);
                        self.mode = Mode::Editing(idx, self.story.markers[idx].make_editor(ctx));
                    }
                }
            }
            Mode::Dragging(ref mut last_pt, idx) => {
                if ctx.redo_mouseover() {
                    if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                        if app.primary.map.get_boundary_polygon().contains_pt(pt) {
                            let dx = pt.x() - last_pt.x();
                            let dy = pt.y() - last_pt.y();
                            *last_pt = pt;
                            self.story.markers[idx] = Marker::new(
                                ctx,
                                self.story.markers[idx]
                                    .pts
                                    .iter()
                                    .map(|pt| pt.offset(dx, dy))
                                    .collect(),
                                self.story.markers[idx].event.clone(),
                            );
                            self.dirty = true;
                            self.redo_panel(ctx);
                        }
                    }
                }

                if ctx.input.key_released(Key::LeftControl) {
                    self.mode = Mode::View;
                }
            }
            Mode::Editing(idx, ref mut panel) => {
                ctx.canvas_movement();
                match panel.event(ctx) {
                    Outcome::Clicked(x) => match x.as_ref() {
                        "close" => {
                            self.mode = Mode::View;
                            self.redo_panel(ctx);
                        }
                        "confirm" => {
                            self.story.markers[idx] = Marker::new(
                                ctx,
                                self.story.markers[idx].pts.clone(),
                                panel.text_box("event"),
                            );
                            self.dirty = true;
                            self.mode = Mode::View;
                            self.redo_panel(ctx);
                        }
                        "delete" => {
                            self.mode = Mode::View;
                            self.hovering = None;
                            self.story.markers.remove(idx);
                            self.dirty = true;
                            self.redo_panel(ctx);
                        }
                        _ => unreachable!(),
                    },
                    _ => {}
                }
            }
            Mode::Freehand(None) => {
                if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                    if ctx.input.left_mouse_button_pressed() {
                        self.mode = Mode::Freehand(Some(Lasso::new(pt)));
                    }
                }
            }
            Mode::Freehand(Some(ref mut lasso)) => {
                if let Some(result) = lasso.event(ctx) {
                    let idx = self.story.markers.len();
                    self.story
                        .markers
                        .push(Marker::new(ctx, result.into_points(), String::new()));
                    self.dirty = true;
                    self.redo_panel(ctx);
                    self.mode = Mode::Editing(idx, self.story.markers[idx].make_editor(ctx));
                }
            }
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    // TODO autosave
                    return Transition::Pop;
                }
                "save" => {
                    if self.story.name == "new story" {
                        return Transition::Push(PromptInput::new(
                            ctx,
                            "Name this story map",
                            Box::new(|name, _, _| {
                                Transition::Multi(vec![
                                    Transition::Pop,
                                    Transition::ModifyState(Box::new(move |state, ctx, app| {
                                        let editor =
                                            state.downcast_mut::<StoryMapEditor>().unwrap();
                                        editor.story.name = name;
                                        editor.story.save(app);
                                        editor.dirty = false;
                                        editor.redo_panel(ctx);
                                    })),
                                ])
                            }),
                        ));
                    } else {
                        self.story.save(app);
                        self.dirty = false;
                        self.redo_panel(ctx);
                    }
                }
                "load" => {
                    // TODO autosave
                    let mut choices = Vec::new();
                    for (name, story) in abstutil::load_all_objects::<RecordedStoryMap>(
                        abstutil::path("player/stories"),
                    ) {
                        if story.name == self.story.name {
                            continue;
                        }
                        if let Some(s) = StoryMap::load(ctx, app, story) {
                            choices.push(Choice::new(name, s));
                        }
                    }
                    choices.push(Choice::new(
                        "new story",
                        StoryMap {
                            name: "new story".to_string(),
                            markers: Vec::new(),
                        },
                    ));

                    return Transition::Push(ChooseSomething::new_below(
                        ctx,
                        self.panel.rect_of("load"),
                        choices,
                        Box::new(|story, _, _| {
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::ModifyState(Box::new(move |state, ctx, _| {
                                    let editor = state.downcast_mut::<StoryMapEditor>().unwrap();
                                    editor.story = story;
                                    editor.dirty = false;
                                    editor.redo_panel(ctx);
                                })),
                            ])
                        }),
                    ));
                }
                "new marker" => {
                    self.hovering = None;
                    self.mode = Mode::PlacingMarker;
                    self.redo_panel(ctx);
                }
                "draw freehand" => {
                    self.hovering = None;
                    self.mode = Mode::Freehand(None);
                    self.redo_panel(ctx);
                }
                "pan" => {
                    self.mode = Mode::View;
                    self.redo_panel(ctx);
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let mut opts = DrawOptions::new();
        opts.label_buildings = true;
        app.draw(g, opts, &ShowEverything::new());

        match self.mode {
            Mode::PlacingMarker => {
                if g.canvas.get_cursor_in_map_space().is_some() {
                    let batch = GeomBatch::load_svg(g, "system/assets/timeline/goal_pos.svg")
                        .centered_on(g.canvas.get_cursor().to_pt())
                        .color(RewriteColor::Change(Color::hex("#5B5B5B"), Color::GREEN));
                    g.fork_screenspace();
                    batch.draw(g);
                    g.unfork();
                }
            }
            Mode::Editing(_, ref panel) => {
                panel.draw(g);
            }
            Mode::Freehand(Some(ref lasso)) => {
                lasso.draw(g);
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

        self.panel.draw(g);
        CommonState::draw_osd(g, app);
    }
}

fn make_panel(ctx: &mut EventCtx, story: &StoryMap, mode: &Mode, dirty: bool) -> Panel {
    Panel::new(Widget::col(vec![
        Widget::row(vec![
            Line("Story map editor").small_heading().draw(ctx),
            Widget::vert_separator(ctx, 30.0),
            Btn::pop_up(ctx, Some(&story.name)).build(ctx, "load", lctrl(Key::L)),
            if dirty {
                Btn::svg_def("system/assets/tools/save.svg").build(ctx, "save", lctrl(Key::S))
            } else {
                Widget::draw_svg_transform(
                    ctx,
                    "system/assets/tools/save.svg",
                    RewriteColor::ChangeAlpha(0.5),
                )
            },
            Btn::close(ctx),
        ]),
        Widget::row(vec![
            if let Mode::PlacingMarker = mode {
                Widget::draw_svg_transform(
                    ctx,
                    "system/assets/timeline/goal_pos.svg",
                    RewriteColor::Change(Color::hex("#5B5B5B"), Color::hex("#4CA7E9")),
                )
            } else {
                Btn::svg_def("system/assets/timeline/goal_pos.svg").build(ctx, "new marker", Key::M)
            },
            if let Mode::View = mode {
                Widget::draw_svg_transform(
                    ctx,
                    "system/assets/tools/pan.svg",
                    RewriteColor::ChangeAll(Color::hex("#4CA7E9")),
                )
            } else {
                Btn::svg_def("system/assets/tools/pan.svg").build(ctx, "pan", Key::Escape)
            },
            match mode {
                Mode::Freehand(_) => Widget::draw_svg_transform(
                    ctx,
                    "system/assets/tools/select.svg",
                    RewriteColor::ChangeAll(Color::hex("#4CA7E9")),
                ),
                _ => Btn::svg_def("system/assets/tools/select.svg").build(
                    ctx,
                    "draw freehand",
                    Key::P,
                ),
            },
        ])
        .evenly_spaced(),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

#[derive(Clone, Serialize, Deserialize)]
struct RecordedStoryMap {
    name: String,
    markers: Vec<(Vec<LonLat>, String)>,
}

struct StoryMap {
    name: String,
    markers: Vec<Marker>,
}

struct Marker {
    pts: Vec<Pt2D>,
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
        for (gps_pts, event) in story.markers {
            let pts = app.primary.map.get_gps_bounds().try_convert(&gps_pts)?;
            markers.push(Marker::new(ctx, pts, event));
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
                        app.primary.map.get_gps_bounds().convert_back(&m.pts),
                        m.event.clone(),
                    )
                })
                .collect(),
        };
        abstutil::write_json(
            abstutil::path(format!("player/stories/{}.json", story.name)),
            &story,
        );
    }
}

impl Marker {
    fn new(ctx: &mut EventCtx, pts: Vec<Pt2D>, event: String) -> Marker {
        let mut batch = GeomBatch::new();

        let hitbox = if pts.len() == 1 {
            batch.append(
                GeomBatch::load_svg(ctx, "system/assets/timeline/goal_pos.svg")
                    .scale(2.0)
                    .centered_on(pts[0])
                    .color(RewriteColor::Change(
                        Color::hex("#5B5B5B"),
                        Color::hex("#FE3D00"),
                    )),
            );
            batch.append(
                Text::from(Line(&event))
                    .with_bg()
                    .render_autocropped(ctx)
                    .scale(0.5)
                    .centered_on(pts[0]),
            );
            batch.unioned_polygon()
        } else {
            let poly = Ring::must_new(pts.clone()).to_polygon();
            batch.push(Color::RED.alpha(0.8), poly.clone());
            if let Ok(o) = poly.to_outline(Distance::meters(1.0)) {
                batch.push(Color::RED, o);
            }
            // TODO Refactor
            batch.append(
                Text::from(Line(&event))
                    .with_bg()
                    .render_autocropped(ctx)
                    .scale(0.5)
                    .centered_on(poly.polylabel()),
            );
            poly
        };
        Marker {
            pts,
            event,
            hitbox,
            draw: ctx.upload(batch),
        }
    }

    fn draw_hovered(&self, g: &mut GfxCtx, app: &App) {
        let mut batch = GeomBatch::new();
        if self.pts.len() == 1 {
            batch.append(
                GeomBatch::load_svg(g, "system/assets/timeline/goal_pos.svg")
                    .scale(2.0)
                    .centered_on(self.pts[0])
                    .color(RewriteColor::Change(Color::hex("#5B5B5B"), app.cs.hovering)),
            );
            batch.append(
                Text::from(Line(&self.event))
                    .with_bg()
                    .render_autocropped(g)
                    .scale(0.75)
                    .centered_on(self.pts[0]),
            );
        } else {
            batch.push(
                app.cs.hovering,
                Ring::must_new(self.pts.clone()).to_polygon(),
            );
            // TODO Refactor plz
            batch.append(
                Text::from(Line(&self.event))
                    .with_bg()
                    .render_autocropped(g)
                    .scale(0.75)
                    .centered_on(self.hitbox.polylabel()),
            );
        }
        batch.draw(g);
    }

    fn make_editor(&self, ctx: &mut EventCtx) -> Panel {
        Panel::new(Widget::col(vec![
            Widget::row(vec![
                Line("Editing marker").small_heading().draw(ctx),
                Btn::close(ctx),
            ]),
            Btn::text_fg("delete").build_def(ctx, None),
            Widget::text_entry(ctx, self.event.clone(), true).named("event"),
            Btn::text_fg("confirm").build_def(ctx, Key::Enter),
        ]))
        .build(ctx)
    }
}

// TODO This should totally be an widgetry tool
// TODO Simplify points
struct Lasso {
    pl: PolyLine,
}

impl Lasso {
    fn new(pt: Pt2D) -> Lasso {
        Lasso {
            pl: PolyLine::must_new(vec![pt, pt.offset(0.1, 0.0)]),
        }
    }

    fn event(&mut self, ctx: &mut EventCtx) -> Option<Ring> {
        if ctx.input.left_mouse_button_released() {
            return Some(simplify(self.pl.points().clone()));
        }
        if ctx.redo_mouseover() {
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                if let Ok(pl) = PolyLine::new(vec![self.pl.last_pt(), pt]) {
                    // Did we make a crossing?
                    if let Some((hit, _)) = self.pl.intersection(&pl) {
                        if let Some(slice) = self.pl.get_slice_starting_at(hit) {
                            return Some(simplify(slice.into_points()));
                        }
                    }

                    let mut pts = self.pl.points().clone();
                    pts.push(pt);
                    if let Ok(new) = PolyLine::new(pts) {
                        self.pl = new;
                    }
                }
            }
        }
        None
    }

    fn draw(&self, g: &mut GfxCtx) {
        g.draw_polygon(
            Color::RED.alpha(0.8),
            self.pl
                .make_polygons(Distance::meters(5.0) / g.canvas.cam_zoom),
        );
    }
}

fn simplify(mut raw: Vec<Pt2D>) -> Ring {
    // TODO This is eating some of the shapes entirely. Wasn't meant for this.
    if false {
        let pts = raw
            .into_iter()
            .map(|pt| lttb::DataPoint::new(pt.x(), pt.y()))
            .collect();
        let mut downsampled = Vec::new();
        for pt in lttb::lttb(pts, 50) {
            downsampled.push(Pt2D::new(pt.x, pt.y));
        }
        downsampled.push(downsampled[0]);
        Ring::must_new(downsampled)
    } else {
        raw.push(raw[0]);
        Ring::must_new(raw)
    }
}
