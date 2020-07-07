use crate::app::{App, ShowEverything};
use crate::common::CommonState;
use crate::game::{State, Transition};
use crate::helpers::{intersections_from_roads, ID};
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, RewriteColor, VerticalAlignment, Widget,
};
use geom::Distance;
use map_model::{IntersectionID, Map, RoadID};
use petgraph::graphmap::UnGraphMap;
use sim::DontDrawAgents;
use std::collections::BTreeSet;

pub struct BulkSelect {
    composite: Composite,
    roads: BTreeSet<RoadID>,
    preview: Option<Drawable>,
    mode: Mode,
    dragging: bool,
}

pub enum Mode {
    Pan,
    Route {
        i1: Option<IntersectionID>,
        preview_path: Option<(IntersectionID, Vec<RoadID>, Drawable)>,
    },
    Paint,
    Erase,
}

impl BulkSelect {
    pub fn new(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State> {
        app.primary.current_selection = None;
        Box::new(BulkSelect {
            composite: make_composite(ctx, app, &Mode::Paint, &BTreeSet::new()),
            roads: BTreeSet::new(),
            preview: None,
            mode: Mode::Paint,
            dragging: false,
        })
    }

    fn roads_changed(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut batch = GeomBatch::new();
        for r in &self.roads {
            batch.push(
                Color::BLUE.alpha(0.5),
                app.primary
                    .map
                    .get_r(*r)
                    .get_thick_polygon(&app.primary.map),
            );
        }
        for i in intersections_from_roads(&self.roads, &app.primary.map) {
            batch.push(
                Color::BLUE.alpha(0.5),
                app.primary.map.get_i(i).polygon.clone(),
            );
        }
        self.preview = Some(ctx.upload(batch));
        self.composite = make_composite(ctx, app, &self.mode, &self.roads);
    }
}

impl State for BulkSelect {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if ctx.redo_mouseover() {
            app.primary.current_selection = app.calculate_current_selection(
                ctx,
                &DontDrawAgents {},
                &ShowEverything::new(),
                false,
                true,
                false,
            );
            match self.mode {
                Mode::Pan => {
                    app.primary.current_selection = None;
                }
                Mode::Route { .. } => {
                    if let Some(ID::Intersection(_)) = app.primary.current_selection {
                    } else {
                        app.primary.current_selection = None;
                    }
                }
                Mode::Paint | Mode::Erase => {
                    if let Some(ID::Road(_)) = app.primary.current_selection {
                    } else if let Some(ID::Lane(l)) = app.primary.current_selection {
                        app.primary.current_selection =
                            Some(ID::Road(app.primary.map.get_l(l).parent));
                    } else {
                        app.primary.current_selection = None;
                    }
                    if let Some(ID::Road(r)) = app.primary.current_selection {
                        if app.primary.map.get_r(r).is_light_rail() {
                            app.primary.current_selection = None;
                        }
                    }
                }
            }
        }

        match self.mode {
            Mode::Pan | Mode::Route { .. } => {
                ctx.canvas_movement();
            }
            Mode::Paint | Mode::Erase => {
                if self.dragging && ctx.input.left_mouse_button_released() {
                    self.dragging = false;
                } else if !self.dragging && ctx.input.left_mouse_button_pressed() {
                    self.dragging = true;
                }
            }
        }

        if self.dragging {
            if let Some(ID::Road(r)) = app.primary.current_selection {
                let change = match self.mode {
                    Mode::Paint => {
                        if self.roads.contains(&r) {
                            false
                        } else {
                            self.roads.insert(r);
                            true
                        }
                    }
                    Mode::Erase => {
                        if self.roads.contains(&r) {
                            self.roads.remove(&r);
                            true
                        } else {
                            false
                        }
                    }
                    Mode::Route { .. } | Mode::Pan => unreachable!(),
                };
                if change {
                    self.roads_changed(ctx, app);
                }
            }
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "paint" => {
                    self.dragging = false;
                    self.mode = Mode::Paint;
                    self.composite = make_composite(ctx, app, &self.mode, &self.roads);
                }
                "erase" => {
                    self.dragging = false;
                    self.mode = Mode::Erase;
                    self.composite = make_composite(ctx, app, &self.mode, &self.roads);
                }
                "pan" => {
                    app.primary.current_selection = None;
                    self.dragging = false;
                    self.mode = Mode::Pan;
                    self.composite = make_composite(ctx, app, &self.mode, &self.roads);
                }
                "select along route" => {
                    app.primary.current_selection = None;
                    self.dragging = false;
                    self.mode = Mode::Route {
                        i1: None,
                        preview_path: None,
                    };
                    self.composite = make_composite(ctx, app, &self.mode, &self.roads);
                }
                "Cancel" => {
                    return Transition::Pop;
                }
                "edit roads" => {
                    return Transition::Replace(crate::edit::bulk::BulkEdit::new(
                        ctx,
                        self.roads.iter().cloned().collect(),
                        self.preview.take().unwrap(),
                    ));
                }
                "export roads to shared-row" => {
                    crate::debug::shared_row::export(
                        self.roads.iter().cloned().collect(),
                        &app.primary.map,
                    );
                }
                _ => unreachable!(),
            },
            None => {}
        }

        if let Mode::Route {
            ref mut i1,
            ref mut preview_path,
        } = self.mode
        {
            if let Some(ID::Intersection(i)) = app.primary.current_selection {
                if i1.is_none() && app.per_obj.left_click(ctx, "start here") {
                    *i1 = Some(i);
                }
            }

            if let Some(i1) = *i1 {
                if let Some(ID::Intersection(i2)) = app.primary.current_selection {
                    if preview_path
                        .as_ref()
                        .map(|(i, _, _)| *i != i2)
                        .unwrap_or(true)
                    {
                        let mut batch = GeomBatch::new();
                        let roads = if let Some(roads) = pathfind(&app.primary.map, i1, i2) {
                            let mut intersections = BTreeSet::new();
                            for r in &roads {
                                let r = app.primary.map.get_r(*r);
                                batch.push(
                                    Color::RED.alpha(0.5),
                                    r.get_thick_polygon(&app.primary.map),
                                );
                                intersections.insert(r.src_i);
                                intersections.insert(r.dst_i);
                            }
                            for i in intersections {
                                batch.push(
                                    Color::RED.alpha(0.5),
                                    app.primary.map.get_i(i).polygon.clone(),
                                );
                            }
                            roads
                        } else {
                            Vec::new()
                        };
                        *preview_path = Some((i2, roads, ctx.upload(batch)));
                    }

                    if preview_path
                        .as_ref()
                        .map(|(_, roads, _)| !roads.is_empty())
                        .unwrap_or(false)
                        && app.per_obj.left_click(ctx, "end here")
                    {
                        let (_, roads, _) = preview_path.take().unwrap();
                        self.roads.extend(roads);
                        self.mode = Mode::Pan;
                        self.roads_changed(ctx, app);
                    }
                } else {
                    *preview_path = None;
                }
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.composite.draw(g);
        if let Some(ref p) = self.preview {
            g.redraw(p);
        }
        if g.canvas.get_cursor_in_map_space().is_some() {
            if let Some(cursor) = match self.mode {
                Mode::Pan => None,
                Mode::Paint => Some("system/assets/tools/pencil.svg"),
                Mode::Erase => Some("system/assets/tools/eraser.svg"),
                Mode::Route { .. } => Some("system/assets/timeline/start_pos.svg"),
            } {
                let mut batch = GeomBatch::new();
                batch.append(
                    GeomBatch::screenspace_svg(g.prerender, cursor)
                        .centered_on(g.canvas.get_cursor().to_pt())
                        .color(RewriteColor::ChangeAll(Color::GREEN)),
                );
                g.fork_screenspace();
                batch.draw(g);
                g.unfork();
            }
        }

        if let Mode::Route {
            ref i1,
            ref preview_path,
        } = self.mode
        {
            if let Some(i) = i1 {
                g.draw_polygon(Color::GREEN, &app.primary.map.get_i(*i).polygon);
            }
            if let Some((_, _, ref p)) = preview_path {
                g.redraw(p);
            }
        }

        CommonState::draw_osd(g, app);
    }
}

fn make_composite(
    ctx: &mut EventCtx,
    app: &App,
    mode: &Mode,
    roads: &BTreeSet<RoadID>,
) -> Composite {
    Composite::new(Widget::col(vec![
        Line("Edit many roads").small_heading().draw(ctx),
        Widget::custom_row(vec![
            if let Mode::Paint = mode {
                Widget::draw_svg_transform(
                    ctx,
                    "system/assets/tools/pencil.svg",
                    RewriteColor::ChangeAll(Color::hex("#4CA7E9")),
                )
            } else {
                Btn::svg_def("system/assets/tools/pencil.svg").build(ctx, "paint", hotkey(Key::P))
            },
            if let Mode::Erase = mode {
                Widget::draw_svg_transform(
                    ctx,
                    "system/assets/tools/eraser.svg",
                    RewriteColor::ChangeAll(Color::hex("#4CA7E9")),
                )
            } else {
                Btn::svg_def("system/assets/tools/eraser.svg").build(
                    ctx,
                    "erase",
                    hotkey(Key::Backspace),
                )
            },
            if let Mode::Route { .. } = mode {
                Widget::draw_svg_transform(
                    ctx,
                    "system/assets/timeline/start_pos.svg",
                    RewriteColor::ChangeAll(Color::hex("#4CA7E9")),
                )
            } else {
                Btn::svg_def("system/assets/timeline/start_pos.svg").build(
                    ctx,
                    "select along route",
                    hotkey(Key::R),
                )
            },
            if let Mode::Pan = mode {
                Widget::draw_svg_transform(
                    ctx,
                    "system/assets/tools/pan.svg",
                    RewriteColor::ChangeAll(Color::hex("#4CA7E9")),
                )
            } else {
                Btn::svg_def("system/assets/tools/pan.svg").build(ctx, "pan", hotkey(Key::Escape))
            },
        ])
        .evenly_spaced(),
        Widget::row(vec![
            if roads.is_empty() {
                Btn::text_fg("Edit 0 roads").inactive(ctx)
            } else {
                Btn::text_fg(format!("Edit {} roads", roads.len())).build(
                    ctx,
                    "edit roads",
                    hotkey(Key::E),
                )
            },
            if app.opts.dev {
                Btn::text_fg(format!("Export {} roads to shared-row", roads.len())).build(
                    ctx,
                    "export roads to shared-row",
                    None,
                )
            } else {
                Widget::nothing()
            },
            Btn::text_fg("Cancel").build_def(ctx, hotkey(Key::Escape)),
        ])
        .evenly_spaced(),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

// Simple search along undirected roads
fn pathfind(map: &Map, i1: IntersectionID, i2: IntersectionID) -> Option<Vec<RoadID>> {
    let mut graph: UnGraphMap<IntersectionID, RoadID> = UnGraphMap::new();
    for r in map.all_roads() {
        if !r.is_light_rail() {
            graph.add_edge(r.src_i, r.dst_i, r.id);
        }
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
