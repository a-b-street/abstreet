use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

use aabb_quadtree::{ItemId, QuadTree};

use geom::{Bounds, Circle, Distance, Polygon, Pt2D};
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx};

pub trait ObjectID: Clone + Copy + Debug + Eq + Hash {
    // Higher shows up in the front.
    fn zorder(&self) -> usize;
}

pub struct Object<ID: ObjectID> {
    id: ID,
    geometry: Vec<(Color, Polygon)>,
}

impl<ID: ObjectID> Object<ID> {
    pub fn new(id: ID, color: Color, poly: Polygon) -> Object<ID> {
        Object {
            id,
            geometry: vec![(color, poly)],
        }
    }
}

struct WorldObject {
    unioned_polygon: Polygon,
    draw: Drawable,
    quadtree_id: ItemId,
}

pub struct World<ID: ObjectID> {
    objects: HashMap<ID, WorldObject>,
    quadtree: QuadTree<ID>,
    current_selection: Option<ID>,
}

impl<ID: ObjectID> World<ID> {
    pub fn new() -> World<ID> {
        World {
            objects: HashMap::new(),
            // Force the quadtree to support any possible positions. Especially when creating
            // synthetic maps, the bounds change, but updating the quadtree is nontrivial. But they
            // have to be non-negative.
            quadtree: QuadTree::default(
                Bounds::from(&vec![
                    Pt2D::new(0.0, 0.0),
                    Pt2D::new(std::f64::MAX, std::f64::MAX),
                ])
                .as_bbox(),
            ),
            current_selection: None,
        }
    }

    pub fn draw<F: Fn(ID) -> bool>(&self, g: &mut GfxCtx, show: F) {
        let mut objects: Vec<ID> = Vec::new();
        for &(id, _, _) in &self.quadtree.query(g.get_screen_bounds().as_bbox()) {
            if show(*id) {
                objects.push(*id);
            }
        }
        objects.sort_by_key(|id| id.zorder());

        for id in objects {
            let obj = &self.objects[&id];
            g.redraw(&obj.draw);
        }

        if let Some(id) = self.current_selection {
            let obj = &self.objects[&id];
            g.draw_polygon(Color::CYAN.alpha(0.5), obj.unioned_polygon.clone());
        }
    }

    pub fn handle_mouseover(&mut self, ctx: &EventCtx) {
        self.current_selection = None;

        let cursor = if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
            pt
        } else {
            return;
        };

        let mut objects: Vec<ID> = Vec::new();
        for &(id, _, _) in &self.quadtree.query(
            Circle::new(cursor, Distance::meters(3.0))
                .get_bounds()
                .as_bbox(),
        ) {
            objects.push(*id);
        }
        objects.sort_by_key(|id| id.zorder());
        objects.reverse();

        for id in objects {
            if self.objects[&id].unioned_polygon.contains_pt(cursor) {
                self.current_selection = Some(id);
                return;
            }
        }
    }

    pub fn force_set_selection(&mut self, id: ID) {
        self.current_selection = Some(id);
    }

    pub fn get_selection(&self) -> Option<ID> {
        self.current_selection
    }

    // TODO This and delete assume the original bounds passed to the quadtree are still valid.
    pub fn add(&mut self, ctx: &EventCtx, obj: Object<ID>) {
        let unioned_polygon =
            Polygon::union_all(obj.geometry.iter().map(|(_, p)| p.clone()).collect());

        let bounds = unioned_polygon.get_bounds();
        // This might break, it might not; the quadtree impl is a little unclear.
        if bounds.min_x < 0.0 || bounds.min_y < 0.0 {
            warn!("{:?} has negative coordinates {:?}", obj.id, bounds);
        }
        let quadtree_id = self.quadtree.insert_with_box(obj.id, bounds.as_bbox());
        let draw = ctx.upload(GeomBatch::from(obj.geometry));
        self.objects.insert(
            obj.id,
            WorldObject {
                unioned_polygon,
                draw,
                quadtree_id,
            },
        );
    }

    pub fn delete(&mut self, id: ID) {
        let obj = self.objects.remove(&id).unwrap();
        self.quadtree.remove(obj.quadtree_id).unwrap();
    }
}
