use crate::AppLike;
use geom::{Circle, Distance, FindClosest, Pt2D, Ring};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{Cached, Color, EventCtx, GfxCtx, Key};

pub struct EditPolygon {
    points: Vec<Pt2D>,
    // The points change size as we zoom out, so rebuild based on cam_zoom
    world: Cached<f64, World<Obj>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Obj {
    Polygon,
    Point(usize),
}
impl ObjectID for Obj {}

impl EditPolygon {
    pub fn new(mut points: Vec<Pt2D>) -> Self {
        if *points.last().unwrap() == points[0] {
            points.pop();
        }
        Self {
            points,
            world: Cached::new(),
        }
    }

    fn rebuild_world(&mut self, ctx: &mut EventCtx, app: &dyn AppLike) {
        let mut world = World::bounded(app.map().get_bounds());

        if self.points.len() >= 3 {
            let mut pts = self.points.to_vec();
            pts.push(pts[0]);
            world
                .add(Obj::Polygon)
                .hitbox(Ring::must_new(pts).into_polygon())
                .zorder(0)
                .draw_color(Color::BLUE.alpha(0.6))
                .hover_alpha(0.3)
                .draggable()
                .build(ctx);
        }

        for (idx, pt) in self.points.iter().enumerate() {
            world
                .add(Obj::Point(idx))
                // Scale the circle as we zoom out
                .hitbox(Circle::new(*pt, Distance::meters(10.0) / ctx.canvas.cam_zoom).to_polygon())
                .zorder(1)
                .draw_color(Color::RED)
                .hover_alpha(0.8)
                .hotkey(Key::Backspace, "delete")
                .draggable()
                .build(ctx);
        }

        world.initialize_hover(ctx);

        if let Some(prev) = self.world.value() {
            world.rebuilt_during_drag(prev);
        }
        self.world.set(ctx.canvas.cam_zoom, world);
    }

    pub fn event(&mut self, ctx: &mut EventCtx, app: &dyn AppLike) {
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
            }
            WorldOutcome::Dragging {
                obj: Obj::Point(idx),
                dx,
                dy,
                ..
            } => {
                self.points[idx] = self.points[idx].offset(dx, dy);
                self.rebuild_world(ctx, app);
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
            }
            WorldOutcome::Keypress("delete", Obj::Point(idx)) => {
                self.points.remove(idx);
                self.rebuild_world(ctx, app);
            }
            _ => {}
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.world.value().unwrap().draw(g);
    }

    pub fn get_points(&self) -> &[Pt2D] {
        &self.points
    }
}
