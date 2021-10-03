use geom::{Circle, Distance, FindClosest, Pt2D};
use sim::TripEndpoint;
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{
    Color, ControlState, CornerRounding, DragDrop, EventCtx, GeomBatch, GfxCtx, Image, Key, Line,
    Outcome, RewriteColor, StackAxis, Text, Widget,
};

use crate::app::App;

/// Click to add waypoints, drag them, see the list on a panel and delete them. The caller owns the
/// Panel, since there's probably more stuff there too.
pub struct InputWaypoints {
    waypoints: Vec<Waypoint>,
    world: World<WaypointID>,
    snap_to_endpts: FindClosest<TripEndpoint>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct WaypointID(usize);
impl ObjectID for WaypointID {}

struct Waypoint {
    at: TripEndpoint,
    label: String,
    center: Pt2D,
}

impl InputWaypoints {
    pub fn new(app: &App) -> InputWaypoints {
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
            world: World::bounded(map.get_bounds()),
            snap_to_endpts,
        }
    }

    pub fn overwrite(&mut self, ctx: &mut EventCtx, app: &App, waypoints: Vec<TripEndpoint>) {
        self.waypoints.clear();
        for at in waypoints {
            self.waypoints.push(Waypoint::new(app, at));
        }
        self.rebuild_world(ctx, app);
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

    /// If the outcome from the panel isn't used by the caller, pass it along here. When this
    /// returns true, something has changed, so the caller may want to update their view of the
    /// route and call `get_panel_widget` again.
    pub fn event(&mut self, ctx: &mut EventCtx, app: &mut App, outcome: Outcome) -> bool {
        match self.world.event(ctx) {
            WorldOutcome::ClickedFreeSpace(pt) => {
                if let Some((at, _)) = self.snap_to_endpts.closest_pt(pt, Distance::meters(30.0)) {
                    self.waypoints.push(Waypoint::new(app, at));
                    self.rebuild_world(ctx, app);
                    return true;
                }
                return false;
            }
            WorldOutcome::Dragging {
                obj: WaypointID(idx),
                cursor,
                ..
            } => {
                if let Some((at, _)) = self
                    .snap_to_endpts
                    .closest_pt(cursor, Distance::meters(30.0))
                {
                    if self.waypoints[idx].at != at {
                        self.waypoints[idx] = Waypoint::new(app, at);
                        self.rebuild_world(ctx, app);
                        return true;
                    }
                }
            }
            WorldOutcome::Keypress("delete", WaypointID(idx)) => {
                self.waypoints.remove(idx);
                self.rebuild_world(ctx, app);
                return true;
            }
            _ => {}
        }

        match outcome {
            Outcome::Clicked(x) => {
                if let Some(x) = x.strip_prefix("delete waypoint ") {
                    let idx = x.parse::<usize>().unwrap();
                    self.waypoints.remove(idx);
                    self.rebuild_world(ctx, app);
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
        self.world.draw(g);
    }

    fn get_waypoint_color(&self, idx: usize) -> Color {
        let total_waypoints = self.waypoints.len();
        match idx {
            0 => Color::GREEN,
            idx if idx == total_waypoints - 1 => Color::RED,
            _ => [Color::BLUE, Color::ORANGE, Color::PURPLE][idx % 3],
        }
    }

    fn rebuild_world(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut world = World::bounded(app.primary.map.get_bounds());

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
                .add(WaypointID(idx))
                .hitbox(hitbox.clone())
                .draw(draw_normal)
                .draw_hover_rewrite(RewriteColor::Change(color, Color::BLUE.alpha(0.5)))
                .hotkey(Key::Backspace, "delete")
                .draggable()
                .build(ctx);
        }

        world.initialize_hover(ctx);
        world.rebuilt_during_drag(&self.world);
        self.world = world;
    }
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
        Waypoint { at, label, center }
    }
}

fn get_waypoint_text(idx: usize) -> char {
    char::from_u32('A' as u32 + idx as u32).unwrap()
}
