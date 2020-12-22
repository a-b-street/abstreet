use std::collections::BTreeSet;

use map_gui::ID;
use map_model::{IntersectionID, RoadID};
use widgetry::{Btn, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Key, RewriteColor, Widget};

use crate::app::App;
use crate::common::{intersections_from_roads, CommonState};

/// A tool for selecting multiple roads.
pub struct RoadSelector {
    pub roads: BTreeSet<RoadID>,
    /// Intersections can't be selected directly. If all roads connected to an intersection are
    /// selected, then the intersection will be too.
    pub intersections: BTreeSet<IntersectionID>,
    pub preview: Option<Drawable>,
    mode: Mode,
    dragging: bool,
}

pub enum Mode {
    /// No selecting, just normal click-and-drag controls.
    Pan,
    /// The user is choosing two intersections, to select the route between.
    Route {
        i1: Option<IntersectionID>,
        preview_path: Option<(IntersectionID, Vec<RoadID>, Drawable)>,
    },
    /// Click and drag to select roads
    Paint,
    /// Click and drag to unselect roads
    Erase,
}

impl RoadSelector {
    pub fn new(ctx: &mut EventCtx, app: &mut App, roads: BTreeSet<RoadID>) -> RoadSelector {
        app.primary.current_selection = None;
        let mut rs = RoadSelector {
            roads,
            intersections: BTreeSet::new(),
            preview: None,
            mode: Mode::Paint,
            dragging: false,
        };
        rs.roads_changed(ctx, app);
        rs
    }

    pub fn make_controls(&self, ctx: &mut EventCtx) -> Widget {
        Widget::custom_row(vec![
            if let Mode::Paint = self.mode {
                Widget::draw_svg_transform(
                    ctx,
                    "system/assets/tools/pencil.svg",
                    RewriteColor::ChangeAll(Color::hex("#4CA7E9")),
                )
            } else {
                Btn::svg_def("system/assets/tools/pencil.svg").build(ctx, "paint", Key::P)
            },
            if let Mode::Erase = self.mode {
                Widget::draw_svg_transform(
                    ctx,
                    "system/assets/tools/eraser.svg",
                    RewriteColor::ChangeAll(Color::hex("#4CA7E9")),
                )
            } else {
                Btn::svg_def("system/assets/tools/eraser.svg").build(ctx, "erase", Key::Backspace)
            },
            if let Mode::Route { .. } = self.mode {
                Widget::draw_svg_transform(
                    ctx,
                    "system/assets/timeline/start_pos.svg",
                    RewriteColor::ChangeAll(Color::hex("#4CA7E9")),
                )
            } else {
                Btn::svg_def("system/assets/timeline/start_pos.svg").build(
                    ctx,
                    "select along route",
                    Key::R,
                )
            },
            if let Mode::Pan = self.mode {
                Widget::draw_svg_transform(
                    ctx,
                    "system/assets/tools/pan.svg",
                    RewriteColor::ChangeAll(Color::hex("#4CA7E9")),
                )
            } else {
                Btn::svg_def("system/assets/tools/pan.svg").build(ctx, "pan", Key::Escape)
            },
        ])
        .evenly_spaced()
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
        self.intersections.clear();
        for i in intersections_from_roads(&self.roads, &app.primary.map) {
            self.intersections.insert(i);
            batch.push(
                Color::BLUE.alpha(0.5),
                app.primary.map.get_i(i).polygon.clone(),
            );
        }
        self.preview = Some(ctx.upload(batch));
    }

    // Pass None. Returns true if anything changed.
    pub fn event(&mut self, ctx: &mut EventCtx, app: &mut App, clicked: Option<&str>) -> bool {
        if ctx.redo_mouseover() {
            match self.mode {
                Mode::Pan => {
                    app.primary.current_selection = None;
                }
                Mode::Route { .. } => {
                    app.primary.current_selection = app.mouseover_unzoomed_intersections(ctx);
                }
                Mode::Paint | Mode::Erase => {
                    app.primary.current_selection =
                        match app.mouseover_unzoomed_roads_and_intersections(ctx) {
                            Some(ID::Road(r)) => Some(r),
                            Some(ID::Lane(l)) => Some(app.primary.map.get_l(l).parent),
                            _ => None,
                        }
                        .and_then(|r| {
                            if app.primary.map.get_r(r).is_light_rail() {
                                None
                            } else {
                                Some(ID::Road(r))
                            }
                        });
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
                    return true;
                }
            }
        }

        match clicked {
            Some(x) => match x {
                "paint" => {
                    self.dragging = false;
                    self.mode = Mode::Paint;
                    return true;
                }
                "erase" => {
                    self.dragging = false;
                    self.mode = Mode::Erase;
                    return true;
                }
                "pan" => {
                    app.primary.current_selection = None;
                    self.dragging = false;
                    self.mode = Mode::Pan;
                    return true;
                }
                "select along route" => {
                    app.primary.current_selection = None;
                    self.dragging = false;
                    self.mode = Mode::Route {
                        i1: None,
                        preview_path: None,
                    };
                    return true;
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
                        let roads = if let Some(roads) = app.primary.map.simple_path_btwn(i1, i2) {
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
                        return true;
                    }
                } else {
                    *preview_path = None;
                }
            }
        }

        false
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &App, show_preview: bool) {
        if show_preview {
            if let Some(ref p) = self.preview {
                g.redraw(p);
            }
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
                    GeomBatch::load_svg(g, cursor)
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
                g.draw_polygon(Color::GREEN, app.primary.map.get_i(*i).polygon.clone());
            }
            if let Some((_, _, ref p)) = preview_path {
                g.redraw(p);
            }
        }

        CommonState::draw_osd(g, app);
    }
}
