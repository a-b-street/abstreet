use geom::{Circle, Distance, FindClosest};
use map_model::{IntersectionID, Map, RoadID};
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, TextExt, Widget};

use crate::app::App;

// TODO Supercede RoadSelector, probably..
pub struct RouteSketcher {
    snap_to_intersections: FindClosest<IntersectionID>,
    route: Route,
    mode: Mode,
    preview: Drawable,
}

impl RouteSketcher {
    pub fn new(ctx: &mut EventCtx, app: &App) -> RouteSketcher {
        let mut snap_to_intersections = FindClosest::new(app.primary.map.get_bounds());
        for i in app.primary.map.all_intersections() {
            snap_to_intersections.add(i.id, i.polygon.points());
        }

        RouteSketcher {
            snap_to_intersections,
            route: Route::new(),
            mode: Mode::Neutral,
            preview: Drawable::empty(ctx),
        }
    }

    fn mouseover_i(&self, ctx: &EventCtx) -> Option<IntersectionID> {
        let pt = ctx.canvas.get_cursor_in_map_space()?;
        // When zoomed really far out, it's harder to click small intersections, so snap more
        // aggressively.
        let threshold = Distance::meters(30.0) / ctx.canvas.cam_zoom;
        let (i, _) = self.snap_to_intersections.closest_pt(pt, threshold)?;
        // After we have a path started, only snap to points on the path to drag them
        if self.route.waypoints.len() > 1
            && !matches!(self.mode, Mode::Dragging { .. })
            && !self.route.full_path.contains(&i)
        {
            return None;
        }
        Some(i)
    }

    fn update_mode(&mut self, ctx: &mut EventCtx, app: &App) {
        match self.mode {
            Mode::Neutral => {
                ctx.canvas_movement();
                if ctx.redo_mouseover() {
                    if let Some(i) = self.mouseover_i(ctx) {
                        self.mode = Mode::Hovering(i);
                    }
                }
            }
            Mode::Hovering(i) => {
                if let Some((_, dy)) = ctx.input.get_mouse_scroll() {
                    ctx.canvas.zoom(dy, ctx.canvas.get_cursor());
                }

                if ctx.normal_left_click() {
                    self.route.add_waypoint(app, i);
                    return;
                }

                if ctx.input.left_mouse_button_pressed() {
                    if let Some(idx) = self.route.idx(i) {
                        self.mode = Mode::Dragging { idx, at: i };
                        return;
                    }
                }

                if ctx.redo_mouseover() {
                    if let Some(i) = self.mouseover_i(ctx) {
                        self.mode = Mode::Hovering(i);
                    } else {
                        self.mode = Mode::Neutral;
                    }
                }
            }
            Mode::Dragging { idx, at } => {
                if ctx.input.left_mouse_button_released() {
                    self.mode = Mode::Hovering(at);
                    return;
                }

                if ctx.redo_mouseover() {
                    if let Some(i) = self.mouseover_i(ctx) {
                        if i != at {
                            let new_idx = self.route.move_waypoint(&app.primary.map, idx, i);
                            self.mode = Mode::Dragging {
                                idx: new_idx,
                                at: i,
                            };
                        }
                    }
                }
            }
        }
    }

    fn update_preview(&mut self, ctx: &mut EventCtx, app: &App) {
        let map = &app.primary.map;
        let mut batch = GeomBatch::new();

        // Draw the confirmed route
        for pair in self.route.full_path.windows(2) {
            // TODO Inefficient!
            let r = map.find_road_between(pair[0], pair[1]).unwrap();
            batch.push(Color::RED.alpha(0.5), map.get_r(r).get_thick_polygon(map));
        }
        for i in &self.route.full_path {
            batch.push(
                Color::BLUE.alpha(0.5),
                Circle::new(map.get_i(*i).polygon.center(), Distance::meters(10.0)).to_polygon(),
            );
        }

        // Debugging
        if false {
            let mut cnt = 0;
            for i in &self.route.waypoints {
                cnt += 1;
                batch.push(
                    Color::RED,
                    Circle::new(map.get_i(*i).polygon.center(), Distance::meters(10.0))
                        .to_polygon(),
                );
                batch.append(
                    widgetry::Text::from(Line(format!("{}", cnt)))
                        .render(ctx)
                        .centered_on(map.get_i(*i).polygon.center()),
                );
            }
        }

        // Draw the current operation
        if let Mode::Hovering(i) = self.mode {
            batch.push(
                Color::BLUE,
                Circle::new(map.get_i(i).polygon.center(), Distance::meters(10.0)).to_polygon(),
            );
            if self.route.waypoints.len() == 1 {
                if let Some((roads, intersections)) =
                    map.simple_path_btwn(self.route.waypoints[0], i)
                {
                    for r in roads {
                        batch.push(Color::BLUE.alpha(0.5), map.get_r(r).get_thick_polygon(map));
                    }
                    for i in intersections {
                        batch.push(Color::BLUE.alpha(0.5), map.get_i(i).polygon.clone());
                    }
                }
            }
        }
        if let Mode::Dragging { at, .. } = self.mode {
            batch.push(
                Color::BLUE,
                Circle::new(map.get_i(at).polygon.center(), Distance::meters(10.0)).to_polygon(),
            );
        }

        self.preview = batch.upload(ctx);
    }

    pub fn get_widget_to_describe(&self, ctx: &mut EventCtx) -> Widget {
        Widget::col(vec![
            if self.route.waypoints.is_empty() {
                "Click to start a route"
            } else if self.route.waypoints.len() == 1 {
                "Click to end the route"
            } else {
                "Click and drag to adjust the route"
            }
            .text_widget(ctx),
            if self.route.waypoints.len() > 1 {
                format!(
                    "{} road segments selected",
                    self.route.full_path.len().max(1) - 1
                )
                .text_widget(ctx)
            } else {
                Widget::nothing()
            },
        ])
    }

    /// True if anything changed
    pub fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> bool {
        let orig_route = self.route.clone();
        let orig_mode = self.mode.clone();
        self.update_mode(ctx, app);
        if self.route != orig_route || self.mode != orig_mode {
            self.update_preview(ctx, app);
            true
        } else {
            false
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.redraw(&self.preview);
        if matches!(self.mode, Mode::Dragging { .. }) {
            if let Some(pt) = g.canvas.get_cursor_in_map_space() {
                g.draw_polygon(
                    Color::BLUE.alpha(0.5),
                    Circle::new(pt, Distance::meters(10.0)).to_polygon(),
                );
            }
        }
    }

    pub fn all_roads(&self, app: &App) -> Vec<RoadID> {
        let mut roads = Vec::new();
        for pair in self.route.full_path.windows(2) {
            // TODO Inefficient!
            roads.push(app.primary.map.find_road_between(pair[0], pair[1]).unwrap());
        }
        roads
    }

    pub fn is_route_started(&self) -> bool {
        self.route.waypoints.len() > 1
    }
}

#[derive(Clone, PartialEq)]
struct Route {
    waypoints: Vec<IntersectionID>,
    full_path: Vec<IntersectionID>,
}

impl Route {
    fn new() -> Route {
        Route {
            waypoints: Vec::new(),
            full_path: Vec::new(),
        }
    }

    fn add_waypoint(&mut self, app: &App, i: IntersectionID) {
        if self.waypoints.is_empty() {
            self.waypoints.push(i);
            assert!(self.full_path.is_empty());
            self.full_path.push(i);
        } else if self.waypoints.len() == 1 && i != self.waypoints[0] {
            if let Some((_, intersections)) = app.primary.map.simple_path_btwn(self.waypoints[0], i)
            {
                self.waypoints.push(i);
                assert_eq!(self.full_path.len(), 1);
                self.full_path = intersections;
            }
        }
        // If there's already two waypoints, can't add more -- can only drag things.
    }

    fn idx(&self, i: IntersectionID) -> Option<usize> {
        self.full_path.iter().position(|x| *x == i)
    }

    // Returns the new full_path index
    fn move_waypoint(&mut self, map: &Map, full_idx: usize, new_i: IntersectionID) -> usize {
        let old_i = self.full_path[full_idx];

        // Move an existing waypoint?
        if let Some(way_idx) = self.waypoints.iter().position(|x| *x == old_i) {
            self.waypoints[way_idx] = new_i;
        } else {
            // Find the next waypoint after this intersection
            for i in &self.full_path[full_idx..] {
                if let Some(way_idx) = self.waypoints.iter().position(|x| x == i) {
                    // Insert a new waypoint before this
                    self.waypoints.insert(way_idx, new_i);
                    break;
                }
            }
        }

        // Recalculate the full path. We could be more efficient and just fix up the part that's
        // changed, but eh.
        self.full_path.clear();
        for pair in self.waypoints.windows(2) {
            // TODO If the new change doesn't work, we could revert.
            let (_, intersections) = map.simple_path_btwn(pair[0], pair[1]).unwrap();
            self.full_path.pop();
            self.full_path.extend(intersections);
        }
        self.idx(new_i).unwrap()
    }
}

#[derive(Clone, PartialEq)]
enum Mode {
    Neutral,
    Hovering(IntersectionID),
    Dragging { idx: usize, at: IntersectionID },
}
