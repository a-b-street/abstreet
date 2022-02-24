use serde::{Deserialize, Serialize};

use geom::{Distance, LonLat, Pt2D, Ring};
use map_gui::render::DrawOptions;
use map_gui::tools::{ChooseSomething, PromptInput};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::tools::Lasso;
use widgetry::{
    lctrl, Choice, Color, DrawBaselayer, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, Panel, SimpleState, State, Text, TextBox, VerticalAlignment, Widget,
};

use crate::app::{App, ShowEverything, Transition};

// Good inspiration: http://sfo-assess.dha.io/, https://github.com/mapbox/storytelling,
// https://storymap.knightlab.com/

/// A simple tool to place markers and free-hand shapes over a map, then label them.
pub struct StoryMapEditor {
    panel: Panel,
    story: StoryMap,
    world: World<MarkerID>,

    dirty: bool,
}

// TODO We'll constantly rebuild the world, so these are indices into a list of markers. Maybe we
// should just assign opaque IDs and hash into them. (Deleting a marker in the middle of the list
// would mean changing IDs of everything after it.)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MarkerID(usize);
impl ObjectID for MarkerID {}

impl StoryMapEditor {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        Self::from_story(ctx, app, StoryMap::new())
    }

    fn from_story(ctx: &mut EventCtx, app: &App, story: StoryMap) -> Box<dyn State<App>> {
        let mut state = StoryMapEditor {
            panel: Panel::empty(ctx),
            story,
            world: World::unbounded(),

            dirty: false,
        };
        state.rebuild_panel(ctx);
        state.rebuild_world(ctx, app);
        Box::new(state)
    }

    fn rebuild_panel(&mut self, ctx: &mut EventCtx) {
        self.panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("Story map editor").small_heading().into_widget(ctx),
                Widget::vert_separator(ctx, 30.0),
                ctx.style()
                    .btn_outline
                    .popup(&self.story.name)
                    .hotkey(lctrl(Key::L))
                    .build_widget(ctx, "load"),
                ctx.style()
                    .btn_plain
                    .icon("system/assets/tools/save.svg")
                    .hotkey(lctrl(Key::S))
                    .disabled(!self.dirty)
                    .build_widget(ctx, "save"),
                ctx.style().btn_close_widget(ctx),
            ]),
            ctx.style()
                .btn_plain
                .icon_text("system/assets/tools/select.svg", "Draw freehand")
                .hotkey(Key::F)
                .build_def(ctx),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx);
    }

    fn rebuild_world(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut world = World::bounded(app.primary.map.get_bounds());

        for (idx, marker) in self.story.markers.iter().enumerate() {
            let mut draw_normal = GeomBatch::new();
            let label_center = if marker.pts.len() == 1 {
                // TODO Erase the "B" from it though...
                draw_normal = map_gui::tools::goal_marker(ctx, marker.pts[0], 2.0);
                marker.pts[0]
            } else {
                let poly = Ring::must_new(marker.pts.clone()).into_polygon();
                draw_normal.push(Color::RED.alpha(0.8), poly.clone());
                if let Ok(o) = poly.to_outline(Distance::meters(1.0)) {
                    draw_normal.push(Color::RED, o);
                }
                poly.polylabel()
            };

            let mut draw_hovered = draw_normal.clone();

            draw_normal.append(
                Text::from(&marker.label)
                    .bg(Color::CYAN)
                    .render_autocropped(ctx)
                    .scale(0.5)
                    .centered_on(label_center),
            );
            let hitbox = draw_normal.unioned_polygon();
            draw_hovered.append(
                Text::from(&marker.label)
                    .bg(Color::CYAN)
                    .render_autocropped(ctx)
                    .scale(0.75)
                    .centered_on(label_center),
            );

            world
                .add(MarkerID(idx))
                .hitbox(hitbox)
                .draw(draw_normal)
                .draw_hovered(draw_hovered)
                .hotkey(Key::Backspace, "delete")
                .clickable()
                .draggable()
                .build(ctx);
        }

        world.initialize_hover(ctx);
        world.rebuilt_during_drag(&self.world);
        self.world = world;
    }
}

impl State<App> for StoryMapEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.world.event(ctx) {
            WorldOutcome::ClickedFreeSpace(pt) => {
                self.story.markers.push(Marker {
                    pts: vec![pt],
                    label: String::new(),
                });
                self.dirty = true;
                self.rebuild_panel(ctx);
                self.rebuild_world(ctx, app);
                return Transition::Push(EditingMarker::new_state(
                    ctx,
                    self.story.markers.len() - 1,
                    "new marker",
                ));
            }
            WorldOutcome::Dragging {
                obj: MarkerID(idx),
                dx,
                dy,
                ..
            } => {
                for pt in &mut self.story.markers[idx].pts {
                    *pt = pt.offset(dx, dy);
                }
                self.dirty = true;
                self.rebuild_panel(ctx);
                self.rebuild_world(ctx, app);
            }
            WorldOutcome::Keypress("delete", MarkerID(idx)) => {
                self.story.markers.remove(idx);
                self.dirty = true;
                self.rebuild_panel(ctx);
                self.rebuild_world(ctx, app);
            }
            WorldOutcome::ClickedObject(MarkerID(idx)) => {
                return Transition::Push(EditingMarker::new_state(
                    ctx,
                    idx,
                    &self.story.markers[idx].label,
                ));
            }
            _ => {}
        }

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    // TODO autosave
                    return Transition::Pop;
                }
                "save" => {
                    if self.story.name == "new story" {
                        return Transition::Push(PromptInput::new_state(
                            ctx,
                            "Name this story map",
                            String::new(),
                            Box::new(|name, _, _| {
                                Transition::Multi(vec![
                                    Transition::Pop,
                                    Transition::ModifyState(Box::new(move |state, ctx, app| {
                                        let editor =
                                            state.downcast_mut::<StoryMapEditor>().unwrap();
                                        editor.story.name = name;
                                        editor.story.save(app);
                                        editor.dirty = false;
                                        editor.rebuild_panel(ctx);
                                    })),
                                ])
                            }),
                        ));
                    } else {
                        self.story.save(app);
                        self.dirty = false;
                        self.rebuild_panel(ctx);
                    }
                }
                "load" => {
                    // TODO autosave
                    let mut choices = Vec::new();
                    for (name, story) in
                        abstio::load_all_objects::<RecordedStoryMap>(abstio::path_player("stories"))
                    {
                        if story.name == self.story.name {
                            continue;
                        }
                        if let Some(s) = StoryMap::load(app, story) {
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

                    return Transition::Push(ChooseSomething::new_state(
                        ctx,
                        "Load story",
                        choices,
                        Box::new(|story, ctx, app| {
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::Replace(StoryMapEditor::from_story(ctx, app, story)),
                            ])
                        }),
                    ));
                }
                "Draw freehand" => {
                    return Transition::Push(Box::new(DrawFreehand {
                        lasso: Lasso::new(),
                        new_idx: self.story.markers.len(),
                    }));
                }
                _ => unreachable!(),
            }
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

        self.panel.draw(g);
        self.world.draw(g);
    }
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
    label: String,
}

impl StoryMap {
    fn new() -> StoryMap {
        StoryMap {
            name: "new story".to_string(),
            markers: Vec::new(),
        }
    }

    fn load(app: &App, story: RecordedStoryMap) -> Option<StoryMap> {
        let mut markers = Vec::new();
        for (gps_pts, label) in story.markers {
            markers.push(Marker {
                pts: app.primary.map.get_gps_bounds().try_convert(&gps_pts)?,
                label,
            });
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
                        m.label.clone(),
                    )
                })
                .collect(),
        };
        abstio::write_json(
            abstio::path_player(format!("stories/{}.json", story.name)),
            &story,
        );
    }
}

struct EditingMarker {
    idx: usize,
}

impl EditingMarker {
    fn new_state(ctx: &mut EventCtx, idx: usize, label: &str) -> Box<dyn State<App>> {
        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("Editing marker").small_heading().into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            ctx.style().btn_outline.text("delete").build_def(ctx),
            TextBox::default_widget(ctx, "label", label.to_string()),
            ctx.style()
                .btn_outline
                .text("confirm")
                .hotkey(Key::Enter)
                .build_def(ctx),
        ]))
        .build(ctx);
        <dyn SimpleState<_>>::new_state(panel, Box::new(EditingMarker { idx }))
    }
}

impl SimpleState<App> for EditingMarker {
    fn on_click(
        &mut self,
        _: &mut EventCtx,
        _: &mut App,
        x: &str,
        panel: &mut Panel,
    ) -> Transition {
        match x {
            "close" => Transition::Pop,
            "confirm" => {
                let idx = self.idx;
                let label = panel.text_box("label");
                Transition::Multi(vec![
                    Transition::Pop,
                    Transition::ModifyState(Box::new(move |state, ctx, app| {
                        let editor = state.downcast_mut::<StoryMapEditor>().unwrap();
                        editor.story.markers[idx].label = label;

                        editor.dirty = true;
                        editor.rebuild_panel(ctx);
                        editor.rebuild_world(ctx, app);
                    })),
                ])
            }
            "delete" => {
                let idx = self.idx;
                Transition::Multi(vec![
                    Transition::Pop,
                    Transition::ModifyState(Box::new(move |state, ctx, app| {
                        let editor = state.downcast_mut::<StoryMapEditor>().unwrap();
                        editor.story.markers.remove(idx);

                        editor.dirty = true;
                        editor.rebuild_panel(ctx);
                        editor.rebuild_world(ctx, app);
                    })),
                ])
            }
            _ => unreachable!(),
        }
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }
}

struct DrawFreehand {
    lasso: Lasso,
    new_idx: usize,
}

impl State<App> for DrawFreehand {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        if let Some(polygon) = self.lasso.event(ctx) {
            let idx = self.new_idx;
            return Transition::Multi(vec![
                Transition::Pop,
                Transition::ModifyState(Box::new(move |state, ctx, app| {
                    let editor = state.downcast_mut::<StoryMapEditor>().unwrap();
                    editor.story.markers.push(Marker {
                        pts: polygon.into_points(),
                        label: String::new(),
                    });

                    editor.dirty = true;
                    editor.rebuild_panel(ctx);
                    editor.rebuild_world(ctx, app);
                })),
                Transition::Push(EditingMarker::new_state(ctx, idx, "new marker")),
            ]);
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.lasso.draw(g);
    }
}
