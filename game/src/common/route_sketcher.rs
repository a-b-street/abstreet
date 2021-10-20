use geom::{Circle, Distance, FindClosest};
use map_model::{IntersectionID, Map, PathConstraints, RoadID};
use widgetry::mapspace::DrawUnzoomedShapes;
use widgetry::{Color, EventCtx, GfxCtx, TextExt, Widget};

use crate::app::App;

const INTERSECTON_RADIUS: Distance = Distance::const_meters(10.0);

// TODO Supercede RoadSelector, probably..
pub struct RouteSketcher {
    snap_to_intersections: FindClosest<IntersectionID>,
    route: Route,
    mode: Mode,
    preview: DrawUnzoomedShapes,
}

impl RouteSketcher {
    pub fn new(app: &App) -> RouteSketcher {
        let mut snap_to_intersections = FindClosest::new(app.primary.map.get_bounds());
        for i in app.primary.map.all_intersections() {
            snap_to_intersections.add(i.id, i.polygon.points());
        }

        RouteSketcher {
            snap_to_intersections,
            route: Route::new(),
            mode: Mode::Neutral,
            preview: DrawUnzoomedShapes::empty(),
        }
    }

    fn mouseover_i(&self, ctx: &EventCtx) -> Option<IntersectionID> {
        let pt = ctx.canvas.get_cursor_in_map_space()?;
        // When zoomed really far out, it's harder to click small intersections, so snap more
        // aggressively. Note this should always be a larger hitbox than how the waypoint circles
        // are drawn.
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

    fn update_preview(&mut self, app: &App) {
        let map = &app.primary.map;
        let mut shapes = DrawUnzoomedShapes::builder();

        // Draw the confirmed route
        for pair in self.route.full_path.windows(2) {
            // TODO Inefficient!
            let r = map.get_r(map.find_road_between(pair[0], pair[1]).unwrap());
            shapes.add_line(r.center_pts.clone(), r.get_width(), Color::RED.alpha(0.5));
        }
        for i in &self.route.full_path {
            shapes.add_circle(
                map.get_i(*i).polygon.center(),
                INTERSECTON_RADIUS,
                Color::BLUE.alpha(0.5),
            );
        }

        // Draw the current operation
        if let Mode::Hovering(i) = self.mode {
            shapes.add_circle(
                map.get_i(i).polygon.center(),
                INTERSECTON_RADIUS,
                Color::BLUE,
            );
            if self.route.waypoints.len() == 1 {
                if let Some((roads, intersections)) =
                    map.simple_path_btwn_v2(self.route.waypoints[0], i, PathConstraints::Car)
                {
                    for r in roads {
                        let r = map.get_r(r);
                        shapes.add_line(
                            r.center_pts.clone(),
                            r.get_width(),
                            Color::BLUE.alpha(0.5),
                        );
                    }
                    for i in intersections {
                        shapes.add_circle(
                            map.get_i(i).polygon.center(),
                            INTERSECTON_RADIUS,
                            Color::BLUE.alpha(0.5),
                        );
                    }
                }
            }
        }
        if let Mode::Dragging { at, .. } = self.mode {
            shapes.add_circle(
                map.get_i(at).polygon.center(),
                INTERSECTON_RADIUS,
                Color::BLUE,
            );
        }

        self.preview = shapes.build();
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

    /// True if the route changed
    pub fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> bool {
        let orig_route = self.route.clone();
        let orig_mode = self.mode.clone();
        self.update_mode(ctx, app);
        if self.route != orig_route || self.mode != orig_mode {
            self.update_preview(app);
            // Only route changes count as a change for the caller, not just hovering on something
            // different
            self.route != orig_route
        } else {
            false
        }
    }

    /// True if something changed. False if this component doesn't even handle that kind of click.
    pub fn on_click(&mut self, x: &str) -> bool {
        if x == "Start over" {
            self.route = Route::new();
            self.mode = Mode::Neutral;
            self.preview = DrawUnzoomedShapes::empty();
            return true;
        }
        false
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.preview.draw(g);
        if matches!(self.mode, Mode::Dragging { .. }) {
            if let Some(pt) = g.canvas.get_cursor_in_map_space() {
                g.draw_polygon(
                    Color::BLUE.alpha(0.5),
                    Circle::new(pt, INTERSECTON_RADIUS).to_polygon(),
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

    /// Has the user even picked a start point?
    pub fn is_route_started(&self) -> bool {
        !self.route.waypoints.is_empty()
    }

    /// Has the user specified a full route?
    pub fn is_route_valid(&self) -> bool {
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
