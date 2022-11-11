use geom::{Circle, Distance, FindClosest, Pt2D};
use map_model::{LaneID, PathConstraints, Position};
use synthpop::TripEndpoint;
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{
    Color, ControlState, CornerRounding, DragDrop, EventCtx, GeomBatch, Image, Key, Line, Outcome,
    RewriteColor, StackAxis, Text, Widget,
};

use crate::AppLike;

/// Click to add waypoints, drag them, see the list on a panel and delete them. The caller owns the
/// Panel and the World, since there's probably more stuff there too.
pub struct InputWaypoints {
    waypoints: Vec<Waypoint>,
    snap_to_main_endpts: FindClosest<TripEndpoint>,
    snap_to_road_endpts: FindClosest<LaneID>,
    max_waypts: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WaypointID(usize);
impl ObjectID for WaypointID {}

struct Waypoint {
    at: TripEndpoint,
    label: String,
    center: Pt2D,
}

impl InputWaypoints {
    /// Allows any number of waypoints
    pub fn new(app: &dyn AppLike, snap_to_lanes_for: Vec<PathConstraints>) -> InputWaypoints {
        let map = app.map();

        let mut snap_to_main_endpts = FindClosest::new(map.get_bounds());
        for i in map.all_intersections() {
            if i.is_border() {
                snap_to_main_endpts.add_polygon(TripEndpoint::Border(i.id), &i.polygon);
            }
        }
        for b in map.all_buildings() {
            snap_to_main_endpts.add_polygon(TripEndpoint::Building(b.id), &b.polygon);
        }

        let mut snap_to_road_endpts = FindClosest::new(map.get_bounds());
        for l in map.all_lanes() {
            if snap_to_lanes_for.iter().any(|c| c.can_use(l, map)) {
                snap_to_road_endpts.add_polygon(l.id, &l.get_thick_polygon());
            }
        }

        InputWaypoints {
            waypoints: Vec::new(),
            snap_to_main_endpts,
            snap_to_road_endpts,
            max_waypts: None,
        }
    }

    /// Only allow drawing routes with 2 waypoints. If a route is loaded with more than that, it
    /// can be modified.
    pub fn new_max_2(app: &dyn AppLike, snap_to_lanes_for: Vec<PathConstraints>) -> Self {
        let mut i = Self::new(app, snap_to_lanes_for);
        i.max_waypts = Some(2);
        i
    }

    /// The caller should call `rebuild_world` after this
    pub fn overwrite(&mut self, app: &dyn AppLike, waypoints: Vec<TripEndpoint>) {
        self.waypoints.clear();
        for at in waypoints {
            self.waypoints.push(Waypoint::new(app, at));
        }
    }

    pub fn get_panel_widget(&self, ctx: &mut EventCtx) -> Widget {
        let mut drag_drop = DragDrop::new(ctx, "waypoint cards", StackAxis::Vertical);
        let mut delete_buttons = Vec::new();

        for (idx, waypt) in self.waypoints.iter().enumerate() {
            let text = get_waypoint_text(idx);
            let icon = {
                let text = Text::from(Line(text).fg(Color::WHITE).bold_body());
                let batch = text.render(ctx);
                let bounds = batch.get_bounds();
                let image = Image::from_batch(batch, bounds)
                    .untinted()
                    .bg_color(self.get_waypoint_color(idx))
                    .padding(10)
                    .dims(16)
                    .corner_rounding(CornerRounding::FullyRounded);
                image
            };

            let waypoint = ctx
                .style()
                .btn_plain
                .text(&waypt.label)
                .image(icon)
                .padding(10);

            let build_batch = |control_state: ControlState| {
                let batch = waypoint.batch(ctx, control_state);
                let bounds = batch.get_bounds();
                let image = Image::from_batch(batch, bounds).untinted();
                image.build_batch(ctx).unwrap()
            };

            let (default_batch, bounds) = build_batch(ControlState::Default);
            let (hovering_batch, _) = build_batch(ControlState::Hovered);
            let (selected_batch, _) = build_batch(ControlState::Hovered);

            drag_drop.push_card(
                idx,
                bounds.into(),
                default_batch,
                hovering_batch,
                selected_batch,
            );

            delete_buttons.push(
                ctx.style()
                    .btn_close()
                    .override_style(&ctx.style().btn_plain_destructive)
                    .build_widget(ctx, &format!("delete waypoint {}", idx)),
            );
        }

        Widget::row(vec![
            drag_drop.into_widget(ctx),
            Widget::custom_col(delete_buttons)
                .evenly_spaced()
                .margin_above(8)
                .margin_below(8),
        ])
    }

    pub fn get_waypoints(&self) -> Vec<TripEndpoint> {
        self.waypoints.iter().map(|w| w.at).collect()
    }

    pub fn len(&self) -> usize {
        self.waypoints.len()
    }

    /// If the outcome from the panel or world isn't used by the caller, pass it along here. When this
    /// returns true, something has changed, so the caller may want to update their view of the
    /// route and call `get_panel_widget` and `rebuild_world` again.
    pub fn event(
        &mut self,
        app: &dyn AppLike,
        panel_outcome: Outcome,
        world_outcome: WorldOutcome<WaypointID>,
    ) -> bool {
        match world_outcome {
            WorldOutcome::ClickedFreeSpace(pt) => {
                if Some(self.waypoints.len()) == self.max_waypts {
                    return false;
                }
                if let Some(at) = self.snap(app, pt) {
                    self.waypoints.push(Waypoint::new(app, at));
                    return true;
                }
                return false;
            }
            WorldOutcome::Dragging {
                obj: WaypointID(idx),
                cursor,
                ..
            } => {
                if let Some(at) = self.snap(app, cursor) {
                    if self.waypoints[idx].at != at {
                        self.waypoints[idx] = Waypoint::new(app, at);
                        return true;
                    }
                }
            }
            WorldOutcome::Keypress("delete", WaypointID(idx)) => {
                self.waypoints.remove(idx);
                return true;
            }
            _ => {}
        }

        match panel_outcome {
            Outcome::Clicked(x) => {
                if let Some(x) = x.strip_prefix("delete waypoint ") {
                    let idx = x.parse::<usize>().unwrap();
                    self.waypoints.remove(idx);
                    return true;
                } else {
                    panic!("Unknown InputWaypoints click {}", x);
                }
            }
            Outcome::DragDropReleased(_, old_idx, new_idx) => {
                self.waypoints.swap(old_idx, new_idx);
                // The order field is baked in, so calculate everything again from scratch
                let waypoints = self.get_waypoints();
                self.overwrite(app, waypoints);
                return true;
            }
            _ => {}
        }

        false
    }

    fn snap(&self, app: &dyn AppLike, cursor: Pt2D) -> Option<TripEndpoint> {
        // Prefer buildings and borders. Some maps have few buildings, or have roads not near
        // buildings, so snap to positions along lanes as a fallback.
        let threshold = Distance::meters(30.0);
        if let Some((at, _)) = self.snap_to_main_endpts.closest_pt(cursor, threshold) {
            return Some(at);
        }
        let (l, _) = self.snap_to_road_endpts.closest_pt(cursor, threshold)?;
        // Try to find the most appropriate position along the lane
        let pl = &app.map().get_l(l).lane_center_pts;
        Some(TripEndpoint::SuddenlyAppear(
            if let Some((dist, _)) = pl.dist_along_of_point(pl.project_pt(cursor)) {
                // Rounding error can snap slightly past the end
                Position::new(l, dist.min(pl.length()))
            } else {
                // If we couldn't figure it out for some reason, just use the middle
                Position::new(l, pl.length() / 2.0)
            },
        ))
    }

    pub fn get_waypoint_color(&self, idx: usize) -> Color {
        let total_waypoints = self.waypoints.len();
        match idx {
            0 => Color::BLACK,
            idx if idx == total_waypoints - 1 => Color::PINK,
            _ => [Color::BLUE, Color::ORANGE, Color::PURPLE][idx % 3],
        }
    }

    /// The caller is responsible for calling `initialize_hover` and `rebuilt_during_drag`.
    pub fn rebuild_world<T: ObjectID, F: Fn(WaypointID) -> T>(
        &self,
        ctx: &mut EventCtx,
        world: &mut World<T>,
        wrap_id: F,
        zorder: usize,
    ) {
        for (idx, waypoint) in self.waypoints.iter().enumerate() {
            let hitbox = Circle::new(waypoint.center, Distance::meters(30.0)).to_polygon();
            let color = self.get_waypoint_color(idx);

            let mut draw_normal = GeomBatch::new();
            draw_normal.push(color, hitbox.clone());
            draw_normal.append(
                Text::from(Line(get_waypoint_text(idx).to_string()).fg(Color::WHITE))
                    .render(ctx)
                    .centered_on(waypoint.center),
            );

            world
                .add(wrap_id(WaypointID(idx)))
                .hitbox(hitbox)
                .zorder(zorder)
                .draw(draw_normal)
                .draw_hover_rewrite(RewriteColor::Change(color, Color::BLUE.alpha(0.5)))
                .hotkey(Key::Backspace, "delete")
                .draggable()
                .build(ctx);
        }
    }
}

impl Waypoint {
    fn new(app: &dyn AppLike, at: TripEndpoint) -> Waypoint {
        let map = app.map();
        let (center, label) = match at {
            TripEndpoint::Building(b) => {
                let b = map.get_b(b);
                (b.polygon.center(), b.address.clone())
            }
            TripEndpoint::Border(i) => {
                let i = map.get_i(i);
                (
                    i.polygon.center(),
                    i.name(app.opts().language.as_ref(), map),
                )
            }
            TripEndpoint::SuddenlyAppear(pos) => (
                pos.pt(map),
                map.get_parent(pos.lane())
                    .get_name(app.opts().language.as_ref()),
            ),
        };
        Waypoint { at, label, center }
    }
}

fn get_waypoint_text(idx: usize) -> char {
    char::from_u32('A' as u32 + idx as u32).unwrap()
}
