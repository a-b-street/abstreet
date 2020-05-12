use crate::app::{App, ShowEverything};
use crate::common::CommonState;
use crate::edit::{apply_map_edits, change_speed_limit};
use crate::game::{State, Transition};
use crate::helpers::ID;
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Distance, Speed};
use map_model::{EditCmd, IntersectionID, Map, RoadID};
use petgraph::graphmap::UnGraphMap;
use sim::DontDrawAgents;
use std::collections::BTreeSet;

// TODO For now, individual turns can't be manipulated. Banning turns could be useful, but I'm not
// sure what to do about the player orphaning a section of the map.
pub struct BulkSelect {
    composite: Composite,
    i1: Option<IntersectionID>,
    preview_path: Option<(IntersectionID, Vec<RoadID>, Drawable)>,
}

impl BulkSelect {
    pub fn new(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State> {
        app.primary.current_selection = None;
        Box::new(BulkSelect {
            composite: Composite::new(
                Widget::col(vec![
                    Line("Edit many roads").small_heading().draw(ctx),
                    "Click one intersection to start"
                        .draw_text(ctx)
                        .named("instructions"),
                    Btn::text_fg("Select roads free-hand / paint mode")
                        .build_def(ctx, hotkey(Key::P))
                        .margin_above(10)
                        .margin_below(5),
                    Btn::text_fg("Quit").build_def(ctx, hotkey(Key::Escape)),
                ])
                .bg(app.cs.panel_bg)
                .padding(10),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            i1: None,
            preview_path: None,
        })
    }
}

impl State for BulkSelect {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.primary.current_selection = app.calculate_current_selection(
                ctx,
                &DontDrawAgents {},
                &ShowEverything::new(),
                false,
                true,
                false,
            );
            if let Some(ID::Intersection(_)) = app.primary.current_selection {
            } else {
                app.primary.current_selection = None;
            }
        }

        if let Some(ID::Intersection(i)) = app.primary.current_selection {
            if self.i1.is_none() && app.per_obj.left_click(ctx, "start here") {
                self.i1 = Some(i);
                self.composite.replace(
                    ctx,
                    "instructions",
                    "Click a second intersection to edit this path".draw_text(ctx),
                );
            }
        }

        if let Some(i1) = self.i1 {
            if let Some(ID::Intersection(i2)) = app.primary.current_selection {
                if self
                    .preview_path
                    .as_ref()
                    .map(|(i, _, _)| *i != i2)
                    .unwrap_or(true)
                {
                    let mut batch = GeomBatch::new();
                    let roads = if let Some(roads) = pathfind(&app.primary.map, i1, i2) {
                        for r in &roads {
                            batch.push(
                                Color::RED,
                                app.primary
                                    .map
                                    .get_r(*r)
                                    .get_thick_polygon(&app.primary.map)
                                    .unwrap(),
                            );
                        }
                        roads
                    } else {
                        Vec::new()
                    };
                    self.preview_path = Some((i2, roads, ctx.upload(batch)));
                }

                if self
                    .preview_path
                    .as_ref()
                    .map(|(_, roads, _)| !roads.is_empty())
                    .unwrap_or(false)
                    && app.per_obj.left_click(ctx, "end here")
                {
                    let (_, roads, preview) = self.preview_path.take().unwrap();
                    return Transition::Replace(BulkEdit::new(ctx, app, roads, preview));
                }
            } else {
                self.preview_path = None;
            }
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Quit" => {
                    return Transition::Pop;
                }
                "Select roads free-hand / paint mode" => {
                    return Transition::Replace(PaintSelect::new(ctx, app));
                }
                _ => unreachable!(),
            },
            None => {}
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.composite.draw(g);
        if let Some(i) = self.i1 {
            g.draw_polygon(Color::GREEN, &app.primary.map.get_i(i).polygon);
        }
        if let Some((_, _, ref p)) = self.preview_path {
            g.redraw(p);
        }
        CommonState::draw_osd(g, app, &None);
    }
}

// Simple search along undirected roads
fn pathfind(map: &Map, i1: IntersectionID, i2: IntersectionID) -> Option<Vec<RoadID>> {
    let mut graph: UnGraphMap<IntersectionID, RoadID> = UnGraphMap::new();
    for r in map.all_roads() {
        graph.add_edge(r.src_i, r.dst_i, r.id);
    }
    let (_, path) = petgraph::algo::astar(
        &graph,
        i1,
        |i| i == i2,
        |(_, _, r)| map.get_r(*r).center_pts.length(),
        |_| Distance::ZERO,
    )?;
    Some(
        path.windows(2)
            .map(|pair| *graph.edge_weight(pair[0], pair[1]).unwrap())
            .collect(),
    )
}

struct BulkEdit {
    composite: Composite,
    roads: Vec<RoadID>,
    preview: Drawable,
}

impl BulkEdit {
    fn new(ctx: &mut EventCtx, app: &App, roads: Vec<RoadID>, preview: Drawable) -> Box<dyn State> {
        Box::new(BulkEdit {
            composite: Composite::new(
                Widget::col(vec![
                    Line(format!("Editing {} roads", roads.len()))
                        .small_heading()
                        .draw(ctx),
                    change_speed_limit(ctx, Speed::miles_per_hour(25.0)).margin_below(5),
                    Widget::row(vec![
                        Btn::text_fg("Cancel").build_def(ctx, None),
                        Btn::text_fg("Confirm change").build_def(ctx, None),
                    ])
                    .centered(),
                ])
                .bg(app.cs.panel_bg)
                .padding(10),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            roads,
            preview,
        })
    }
}

impl State for BulkEdit {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Cancel" => {
                    return Transition::Pop;
                }
                "Confirm change" => {
                    let speed = self.composite.dropdown_value("speed limit");
                    let mut edits = app.primary.map.get_edits().clone();
                    for r in &self.roads {
                        edits.commands.push(EditCmd::ChangeSpeedLimit {
                            id: *r,
                            new: speed,
                            old: app.primary.map.get_r(*r).speed_limit,
                        });
                    }
                    apply_map_edits(ctx, app, edits);
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.composite.draw(g);
        g.redraw(&self.preview);
    }
}

struct PaintSelect {
    composite: Composite,
    roads: BTreeSet<RoadID>,
    preview: Option<Drawable>,

    select_key_held: bool,
    deselect_key_held: bool,
}

impl PaintSelect {
    pub fn new(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State> {
        Box::new(PaintSelect {
            composite: Composite::new(
                Widget::col(vec![
                    Text::from_multiline(vec![
                        Line("Edit many roads").small_heading(),
                        Line("Hold the left shift key and move your mouse over a road to select"),
                        Line("or hold left control to deselect roads"),
                    ])
                    .draw(ctx)
                    .margin_below(5),
                    Btn::text_fg("Edit these roads").build_def(ctx, None),
                    Btn::text_fg("Quit").build_def(ctx, hotkey(Key::Escape)),
                ])
                .bg(app.cs.panel_bg)
                .padding(10),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            roads: BTreeSet::new(),
            preview: None,
            select_key_held: false,
            deselect_key_held: false,
        })
    }
}

impl State for PaintSelect {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        // TODO Changing cursor could be cool
        if self.select_key_held {
            self.select_key_held = !ctx.input.key_released(Key::LeftShift);
        } else {
            self.select_key_held = ctx
                .input
                .unimportant_key_pressed(Key::LeftShift, "hold to select roads");
        }
        if self.deselect_key_held {
            self.deselect_key_held = !ctx.input.key_released(Key::LeftControl);
        } else {
            self.deselect_key_held = ctx
                .input
                .unimportant_key_pressed(Key::LeftControl, "hold to deselect roads");
        }

        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.primary.current_selection = app.calculate_current_selection(
                ctx,
                &DontDrawAgents {},
                &ShowEverything::new(),
                false,
                true,
                false,
            );
            if let Some(ID::Road(_)) = app.primary.current_selection {
            } else {
                app.primary.current_selection = None;
            }
        }

        if let Some(ID::Road(r)) = app.primary.current_selection {
            let change = if self.select_key_held {
                if self.roads.contains(&r) {
                    false
                } else {
                    self.roads.insert(r);
                    true
                }
            } else if self.deselect_key_held {
                if self.roads.contains(&r) {
                    self.roads.remove(&r);
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if change {
                let mut batch = GeomBatch::new();
                for r in &self.roads {
                    batch.push(
                        Color::BLUE,
                        app.primary
                            .map
                            .get_r(*r)
                            .get_thick_polygon(&app.primary.map)
                            .unwrap(),
                    );
                }
                self.preview = Some(ctx.upload(batch));
            }
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Quit" => {
                    return Transition::Pop;
                }
                "Edit these roads" => {
                    if self.roads.is_empty() {
                        return Transition::Pop;
                    }
                    return Transition::Replace(BulkEdit::new(
                        ctx,
                        app,
                        self.roads.iter().cloned().collect(),
                        self.preview.take().unwrap(),
                    ));
                }
                _ => unreachable!(),
            },
            None => {}
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.composite.draw(g);
        if let Some(ref p) = self.preview {
            g.redraw(p);
        }
    }
}
