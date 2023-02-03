use anyhow::Result;

use crate::AppLike;
use geom::{Angle, ArrowCap, Circle, Distance, FindClosest, Line, PolyLine, Pt2D, Ring};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{Cached, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Key};

// TODO Callers may want to explain the controls -- the D key for the leafblower, in particular.
pub struct EditPolygon {
    points: Vec<Pt2D>,
    // The points change size as we zoom out, so rebuild based on cam_zoom
    world: Cached<f64, World<Obj>>,
    polygon_draggable: bool,

    leafblower: Leafblower,
}

struct Leafblower {
    cursor: Option<Pt2D>,
    direction: Option<Angle>,
    draw: Drawable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Obj {
    Polygon,
    Point(usize),
}
impl ObjectID for Obj {}

impl EditPolygon {
    pub fn new(
        ctx: &EventCtx,
        app: &dyn AppLike,
        mut points: Vec<Pt2D>,
        polygon_draggable: bool,
    ) -> Self {
        if !points.is_empty() && *points.last().unwrap() == points[0] {
            points.pop();
        }
        let mut edit = Self {
            points,
            world: Cached::new(),
            polygon_draggable,
            leafblower: Leafblower {
                cursor: ctx.canvas.get_cursor_in_map_space(),
                direction: None,
                draw: Drawable::empty(ctx),
            },
        };
        edit.rebuild_world(ctx, app);
        edit
    }

    fn add_polygon_to_world(&self, ctx: &EventCtx, world: &mut World<Obj>) {
        if self.points.len() >= 3 {
            let mut pts = self.points.to_vec();
            pts.push(pts[0]);
            if let Ok(ring) = Ring::new(pts) {
                let obj = world
                    .add(Obj::Polygon)
                    .hitbox(ring.into_polygon())
                    .zorder(0)
                    .draw_color(Color::BLUE.alpha(0.6));
                if self.polygon_draggable {
                    obj.hover_alpha(0.3).draggable().build(ctx);
                } else {
                    obj.build(ctx);
                }
            }
        }
    }

    fn rebuild_world(&mut self, ctx: &EventCtx, app: &dyn AppLike) {
        let mut world = World::bounded(app.map().get_bounds());

        self.add_polygon_to_world(ctx, &mut world);

        // Scale the circle as we zoom out
        let circle =
            Circle::new(Pt2D::zero(), Distance::meters(10.0) / ctx.canvas.cam_zoom).to_polygon();
        for (idx, pt) in self.points.iter().enumerate() {
            world
                .add(Obj::Point(idx))
                .hitbox(circle.translate(pt.x(), pt.y()))
                .zorder(1)
                .draw_color(Color::RED)
                .hover_alpha(0.8)
                .hotkey(Key::Backspace, "delete")
                .draggable()
                .build(ctx);
        }

        world.initialize_hover(ctx);

        if let Some(prev) = self.world.value() {
            world.rebuilt_during_drag(ctx, prev);
        }
        self.world.set(ctx.canvas.cam_zoom, world);
    }

    fn rebuild_one_point(&mut self, ctx: &EventCtx, idx: usize) {
        let (_, mut world) = self.world.take().unwrap();

        world.delete_before_replacement(Obj::Polygon);
        self.add_polygon_to_world(ctx, &mut world);

        // Change the point
        // TODO Some repeated code, but meh
        world.delete_before_replacement(Obj::Point(idx));
        let circle =
            Circle::new(Pt2D::zero(), Distance::meters(10.0) / ctx.canvas.cam_zoom).to_polygon();
        world
            .add(Obj::Point(idx))
            .hitbox(circle.translate(self.points[idx].x(), self.points[idx].y()))
            .zorder(1)
            .draw_color(Color::RED)
            .hover_alpha(0.8)
            .hotkey(Key::Backspace, "delete")
            .draggable()
            .build(ctx);

        self.world.set(ctx.canvas.cam_zoom, world);
    }

    /// True if the polygon is modified
    pub fn event(&mut self, ctx: &mut EventCtx, app: &dyn AppLike) -> bool {
        // Recalculate if zoom has changed
        if self.world.key() != Some(ctx.canvas.cam_zoom) {
            self.rebuild_world(ctx, app);
        }

        match self.world.value_mut().unwrap().event(ctx) {
            WorldOutcome::ClickedFreeSpace(pt) => {
                // Insert the new point in the "middle" of the closest line segment
                let mut closest = FindClosest::new(app.map().get_bounds());
                for (idx, pair) in self.points.windows(2).enumerate() {
                    closest.add(idx + 1, &[pair[0], pair[1]]);
                }
                if let Some((idx, _)) = closest.closest_pt(pt, Distance::meters(1000.0)) {
                    self.points.insert(idx, pt);
                } else {
                    // Just put on the end
                    self.points.push(pt);
                }

                self.rebuild_world(ctx, app);
                true
            }
            WorldOutcome::Dragging {
                obj: Obj::Point(idx),
                dx,
                dy,
                ..
            } => {
                self.points[idx] = self.points[idx].offset(dx, dy);
                self.rebuild_one_point(ctx, idx);
                true
            }
            WorldOutcome::Dragging {
                obj: Obj::Polygon,
                dx,
                dy,
                ..
            } => {
                for pt in &mut self.points {
                    *pt = pt.offset(dx, dy);
                }
                self.rebuild_world(ctx, app);
                true
            }
            WorldOutcome::Keypress("delete", Obj::Point(idx)) => {
                self.points.remove(idx);
                self.rebuild_world(ctx, app);
                true
            }
            _ => {
                // TODO Does World eat the mouse moved event?
                if self.leafblower.event(
                    ctx,
                    &mut self.points,
                    self.world.value().unwrap().get_hovering().is_some(),
                ) {
                    self.rebuild_world(ctx, app);
                    return true;
                }
                false
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.world.value().unwrap().draw(g);
        g.redraw(&self.leafblower.draw);
    }

    /// Could fail if the user edits the ring and makes it invalid
    pub fn get_ring(&self) -> Result<Ring> {
        let mut pts = self.points.clone();
        pts.push(pts[0]);
        Ring::new(pts)
    }
}

impl Leafblower {
    fn event(&mut self, ctx: &mut EventCtx, points: &mut Vec<Pt2D>, suppress: bool) -> bool {
        let mut cursor = ctx.canvas.get_cursor_in_map_space();
        if suppress {
            cursor = None;
        }
        if self.cursor != cursor {
            self.cursor = cursor;
            self.update(ctx, points);
        }

        if ctx.input.pressed(Key::D) {
            if let Some(angle) = self.direction {
                let cursor = self.cursor.unwrap();
                let threshold = Distance::meters(100.0) / ctx.canvas.cam_zoom;
                for pt in points.iter_mut() {
                    if pt.dist_to(cursor) <= threshold {
                        *pt = pt.project_away(0.1 * threshold, angle);
                    }
                }
                // Force an update
                self.cursor = None;
                self.update(ctx, points);
                return true;
            }
        }

        false
    }

    fn update(&mut self, ctx: &EventCtx, points: &[Pt2D]) {
        self.direction = None;
        self.draw = Drawable::empty(ctx);

        // TODO Express in pixels?
        let threshold = Distance::meters(100.0) / ctx.canvas.cam_zoom;

        let cursor = if let Some(pt) = self.cursor {
            pt
        } else {
            return;
        };

        let mut angles = Vec::new();
        for pt in points {
            if let Ok(line) = Line::new(cursor, *pt) {
                if line.length() <= threshold {
                    angles.push(line.angle());
                }
            }
        }
        if !angles.is_empty() {
            self.direction = Some(Angle::average(angles));
            self.draw = GeomBatch::from(vec![(
                Color::BLACK,
                PolyLine::must_new(vec![
                    cursor,
                    cursor.project_away(threshold, self.direction.unwrap()),
                ])
                .make_arrow(
                    Distance::meters(10.0) / ctx.canvas.cam_zoom,
                    ArrowCap::Triangle,
                ),
            )])
            .upload(ctx);
        }
    }
}
