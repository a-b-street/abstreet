use geom::{Circle, Distance, FindClosest, Polygon, Pt2D};
use sim::TripEndpoint;
use widgetry::{
    Color, ControlState, CornerRounding, DragDrop, Drawable, EventCtx, GeomBatch, GfxCtx, Image,
    Line, Outcome, StackAxis, Text, Widget,
};

use crate::app::App;

/// Click to add waypoints, drag them, see the list on a panel and delete them. The caller owns the
/// Panel, since there's probably more stuff there too.
pub struct InputWaypoints {
    waypoints: Vec<Waypoint>,
    draw_waypoints: Drawable,
    hovering_on_waypt: Option<usize>,
    draw_hover: Drawable,
    // TODO Invariant not captured by these separate fields: when dragging is true,
    // hovering_on_waypt is fixed.
    dragging: bool,
    snap_to_endpts: FindClosest<TripEndpoint>,
}

// TODO Maybe it's been a while and I've forgotten some UI patterns, but this is painfully manual.
// I think we need a draggable map-space thing.
struct Waypoint {
    at: TripEndpoint,
    label: String,
    hitbox: Polygon,
    center: Pt2D,
}

impl InputWaypoints {
    pub fn new(ctx: &mut EventCtx, app: &App) -> InputWaypoints {
        let map = &app.primary.map;
        let mut snap_to_endpts = FindClosest::new(map.get_bounds());
        for i in map.all_intersections() {
            if i.is_border() {
                snap_to_endpts.add(TripEndpoint::Border(i.id), i.polygon.points());
            }
        }
        for b in map.all_buildings() {
            snap_to_endpts.add(TripEndpoint::Bldg(b.id), b.polygon.points());
        }

        InputWaypoints {
            waypoints: Vec::new(),
            draw_waypoints: Drawable::empty(ctx),
            hovering_on_waypt: None,
            draw_hover: Drawable::empty(ctx),
            dragging: false,
            snap_to_endpts,
        }
    }

    pub fn overwrite(&mut self, ctx: &mut EventCtx, app: &App, waypoints: Vec<TripEndpoint>) {
        self.waypoints.clear();
        for at in waypoints {
            self.waypoints.push(Waypoint::new(app, at));
        }
        self.update_waypoints_drawable(ctx);
        self.update_hover(ctx);
    }

    pub fn get_panel_widget(&self, ctx: &mut EventCtx) -> Widget {
        let mut drag_drop = DragDrop::new(ctx, "waypoint cards", StackAxis::Vertical);
        let mut delete_buttons = Vec::new();

        for (idx, waypt) in self.waypoints.iter().enumerate() {
            let order = char::from_u32('A' as u32 + idx as u32).unwrap();
            let icon = {
                let text = Text::from(Line(order.to_string()).fg(Color::WHITE).bold_body());
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

        Widget::col(vec![
            Widget::row(vec![
                drag_drop.into_widget(ctx),
                Widget::custom_col(delete_buttons)
                    .evenly_spaced()
                    .margin_above(8)
                    .margin_below(8),
            ]),
            Widget::row(vec![
                Image::from_path("system/assets/tools/mouse.svg").into_widget(ctx),
                Text::from_all(vec![
                    Line("Click").fg(ctx.style().text_hotkey_color),
                    Line(" to add a waypoint, "),
                    Line("drag").fg(ctx.style().text_hotkey_color),
                    Line(" a waypoint to move it"),
                ])
                .into_widget(ctx),
            ]),
        ])
    }

    pub fn get_waypoints(&self) -> Vec<TripEndpoint> {
        self.waypoints.iter().map(|w| w.at).collect()
    }

    /// If the outcome from the panel isn't used by the caller, pass it along here. This handles
    /// calling `ctx.canvas_movement` when appropriate. When this returns true, something has
    /// changed, so the caller may want to update their view of the route and call
    /// `get_panel_widget` again.
    pub fn event(&mut self, ctx: &mut EventCtx, app: &mut App, outcome: Outcome) -> bool {
        if self.dragging {
            if ctx.redo_mouseover() && self.update_dragging(ctx, app) == Some(true) {
                return true;
            }
            if ctx.input.left_mouse_button_released() {
                self.dragging = false;
                self.update_hover(ctx);
            }
        } else {
            if ctx.redo_mouseover() {
                self.update_hover(ctx);
            }

            if self.hovering_on_waypt.is_none() {
                ctx.canvas_movement();
            } else if let Some((_, dy)) = ctx.input.get_mouse_scroll() {
                // Zooming is OK, but can't start click and drag
                ctx.canvas.zoom(dy, ctx.canvas.get_cursor());
            }

            if self.hovering_on_waypt.is_some() && ctx.input.left_mouse_button_pressed() {
                self.dragging = true;
            }

            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                if self.hovering_on_waypt.is_none() && ctx.normal_left_click() {
                    if let Some((at, _)) =
                        self.snap_to_endpts.closest_pt(pt, Distance::meters(30.0))
                    {
                        self.waypoints.push(Waypoint::new(app, at));
                        self.update_waypoints_drawable(ctx);
                        self.update_hover(ctx);
                        return true;
                    }
                }
            }
        }

        match outcome {
            Outcome::Clicked(x) => {
                if let Some(x) = x.strip_prefix("delete waypoint ") {
                    let idx = x.parse::<usize>().unwrap();
                    self.waypoints.remove(idx);
                    // Recalculate labels, in case we deleted in the middle
                    for waypt in self.waypoints.iter_mut() {
                        *waypt = Waypoint::new(app, waypt.at);
                    }

                    self.update_waypoints_drawable(ctx);
                    return true;
                } else {
                    panic!("Unknown InputWaypoints click {}", x);
                }
            }
            Outcome::DragDropReleased(_, old_idx, new_idx) => {
                self.waypoints.swap(old_idx, new_idx);
                // The order field is baked in, so calculate everything again from scratch
                let waypoints = self.get_waypoints();
                self.overwrite(ctx, app, waypoints);
                return true;
            }
            _ => {}
        }

        false
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.redraw(&self.draw_waypoints);
        g.redraw(&self.draw_hover);
    }

    fn get_waypoint_color(&self, idx: usize) -> Color {
        let total_waypoints = self.waypoints.len();
        let wp = {
            match idx {
                0 => WaypointPosition::Start,
                idx if idx == total_waypoints - 1 => WaypointPosition::End,
                // technically this includes the case where idx >= total_waypoints which should hopefully never happen
                _ => WaypointPosition::Middle,
            }
        };

        match wp {
            WaypointPosition::Start => Color::GREEN,
            WaypointPosition::End => Color::RED,
            WaypointPosition::Middle => [Color::BLUE, Color::ORANGE, Color::PURPLE][idx % 3],
        }
    }

    fn update_waypoints_drawable(&mut self, ctx: &mut EventCtx) {
        let mut batch = GeomBatch::new();
        for (idx, waypt) in &mut self.waypoints.iter().enumerate() {
            let geom = {
                let color = self.get_waypoint_color(idx);

                let mut geom = GeomBatch::new();

                geom.push(color, waypt.hitbox.clone());

                let order = char::from_u32('A' as u32 + idx as u32).unwrap();
                geom.append(
                    Text::from(Line(format!("{}", order)).fg(Color::WHITE))
                        .render(ctx)
                        .centered_on(waypt.center),
                );

                geom
            };
            batch.append(geom);
        }
        self.draw_waypoints = ctx.upload(batch);
    }

    fn update_hover(&mut self, ctx: &EventCtx) {
        self.hovering_on_waypt = None;

        if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
            self.hovering_on_waypt = self
                .waypoints
                .iter()
                .position(|waypt| waypt.hitbox.contains_pt(pt));
        }

        let mut batch = GeomBatch::new();
        if let Some(idx) = self.hovering_on_waypt {
            batch.push(Color::BLUE.alpha(0.5), self.waypoints[idx].hitbox.clone());
        }
        self.draw_hover = ctx.upload(batch);
    }

    // `Some(true)` means to update.
    fn update_dragging(&mut self, ctx: &mut EventCtx, app: &App) -> Option<bool> {
        let pt = ctx.canvas.get_cursor_in_map_space()?;
        let (at, _) = self.snap_to_endpts.closest_pt(pt, Distance::meters(30.0))?;

        let mut changed = false;
        let idx = self.hovering_on_waypt.unwrap();
        if self.waypoints[idx].at != at {
            self.waypoints[idx] = Waypoint::new(app, at);
            self.update_waypoints_drawable(ctx);
            changed = true;
        }

        let mut batch = GeomBatch::new();
        // Show where we're currently snapped
        batch.push(Color::BLUE.alpha(0.5), self.waypoints[idx].hitbox.clone());
        self.draw_hover = ctx.upload(batch);

        Some(changed)
    }
}

enum WaypointPosition {
    Start,
    Middle,
    End,
}

impl Waypoint {
    fn new(app: &App, at: TripEndpoint) -> Waypoint {
        let map = &app.primary.map;
        let (center, label) = match at {
            TripEndpoint::Bldg(b) => {
                let b = map.get_b(b);
                (b.polygon.center(), b.address.clone())
            }
            TripEndpoint::Border(i) => {
                let i = map.get_i(i);
                (i.polygon.center(), i.name(app.opts.language.as_ref(), map))
            }
            TripEndpoint::SuddenlyAppear(pos) => (pos.pt(map), pos.to_string()),
        };

        let hitbox = Circle::new(center, Distance::meters(30.0)).to_polygon();

        Waypoint {
            at,
            label,
            hitbox,
            center,
        }
    }
}
