use geom::{Circle, Distance, FindClosest};
use map_model::{IntersectionID, Map, PathConstraints, RoadID};
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, TextExt, Widget};

use crate::app::App;

const RADIUS: Distance = Distance::const_meters(10.0);

// TODO Supercede RoadSelector, probably..
pub struct RouteSketcher {
    snap_to_intersections: FindClosest<IntersectionID>,
    route: Route,
    mode: Mode,
    preview: Drawable,

    thickness: f64,
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

            thickness: 0.0,
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
                if ctx.input.left_mouse_button_pressed() {
                    if let Some(idx) = self.route.idx(i) {
                        self.mode = Mode::Dragging { idx, at: i };
                        return;
                    }
                }

                ctx.canvas_movement();

                if ctx.normal_left_click() {
                    self.route.add_waypoint(app, i);
                    return;
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
        let thickness = zoom_to_thickness(ctx);
        let mut batch = GeomBatch::new();

        // Draw the confirmed route
        for pair in self.route.full_path.windows(2) {
            // TODO Inefficient!
            let r = map.get_r(map.find_road_between(pair[0], pair[1]).unwrap());
            batch.push(
                Color::RED.alpha(0.5),
                r.center_pts.make_polygons(thickness * r.get_width()),
            );
        }
        for i in &self.route.full_path {
            batch.push(
                Color::BLUE.alpha(0.5),
                Circle::new(map.get_i(*i).polygon.center(), thickness * RADIUS).to_polygon(),
            );
        }

        // Debugging
        if false {
            let mut cnt = 0;
            for i in &self.route.waypoints {
                cnt += 1;
                batch.push(
                    Color::RED,
                    Circle::new(map.get_i(*i).polygon.center(), thickness * RADIUS).to_polygon(),
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
                Circle::new(map.get_i(i).polygon.center(), thickness * RADIUS).to_polygon(),
            );
            if self.route.waypoints.len() == 1 {
                if let Some((roads, intersections)) =
                    map.simple_path_btwn_v2(self.route.waypoints[0], i, PathConstraints::Car)
                {
                    for r in roads {
                        batch.push(Color::BLUE.alpha(0.5), map.get_r(r).get_thick_polygon());
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
                Circle::new(map.get_i(at).polygon.center(), thickness * RADIUS).to_polygon(),
            );
        }

        self.preview = batch.upload(ctx);
        self.thickness = thickness;
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
            if self.route.waypoints.is_empty() {
                Widget::nothing()
            } else {
                ctx.style()
                    .btn_plain_destructive
                    .text("Start over")
                    .build_def(ctx)
            },
        ])
    }

    /// True if anything changed
    pub fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> bool {
        let orig_route = self.route.clone();
        let orig_mode = self.mode.clone();
        self.update_mode(ctx, app);
        println!(
            "Current thickness {}, zoom_to_thickness({}) now {}",
            self.thickness,
            ctx.canvas.cam_zoom,
            zoom_to_thickness(ctx)
        );
        if self.route != orig_route
            || self.mode != orig_mode
            || zoom_to_thickness(ctx) != self.thickness
        {
            self.update_preview(ctx, app);
            true
        } else {
            false
        }
    }

    /// True if something changed. False if this component doesn't even handle that kind of click.
    pub fn on_click(&mut self, ctx: &EventCtx, x: &str) -> bool {
        if x == "Start over" {
            self.route = Route::new();
            self.mode = Mode::Neutral;
            self.preview = Drawable::empty(ctx);
            return true;
        }
        false
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.redraw(&self.preview);
        if matches!(self.mode, Mode::Dragging { .. }) {
            if let Some(pt) = g.canvas.get_cursor_in_map_space() {
                g.draw_polygon(
                    Color::BLUE.alpha(0.5),
                    Circle::new(pt, self.thickness * RADIUS).to_polygon(),
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
            // Route for cars, because we're doing this to transform roads meant for cars. We could
            // equivalently use Bike in most cases, except for highways where biking is currently
            // banned. This tool could be used to carve out space and allow that.
            if let Some((_, intersections)) =
                app.primary
                    .map
                    .simple_path_btwn_v2(self.waypoints[0], i, PathConstraints::Car)
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

        // Edge case when we've placed just one point, then try to drag it
        if self.waypoints.len() == 1 {
            assert_eq!(self.waypoints[0], old_i);
            self.waypoints = vec![new_i];
            self.full_path = vec![new_i];
            return 0;
        }

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
            let (_, intersections) = map
                .simple_path_btwn_v2(pair[0], pair[1], PathConstraints::Car)
                .unwrap();
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

fn zoom_to_thickness(ctx: &EventCtx) -> f64 {
    let zoom = ctx.canvas.cam_zoom;
    let bucketed_zoom = if zoom >= 1.0 {
        1.0
    } else {
        (zoom * 10.0).round() / 10.0
    };

    // Thicker lines as we zoom out. Scale up to 5x. Never shrink past the road's actual width.
    let thickness = (0.5 / bucketed_zoom).max(1.0);
    // And on gigantic maps, zoom may approach 0, so avoid NaNs.
    if thickness.is_finite() {
        thickness
    } else {
        5.0
    }
}
