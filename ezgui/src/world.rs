use crate::{Color, Drawable, EventCtx, GfxCtx, Prerender, Text};
use aabb_quadtree::{ItemId, QuadTree};
use geom::{Bounds, Circle, Distance, Polygon};
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

pub trait ObjectID: Clone + Copy + Debug + Eq + Hash {
    // Higher shows up in the front.
    fn zorder(&self) -> usize;
}

struct Object {
    polygon: Polygon,
    draw: Drawable,
    info: Text,
    quadtree_id: ItemId,
}

pub struct World<ID: ObjectID> {
    objects: HashMap<ID, Object>,
    quadtree: QuadTree<ID>,
    current_selection: Option<ID>,
}

impl<ID: ObjectID> World<ID> {
    pub fn new(bounds: &Bounds) -> World<ID> {
        World {
            objects: HashMap::new(),
            quadtree: QuadTree::default(bounds.as_bbox()),
            current_selection: None,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        let mut objects: Vec<ID> = Vec::new();
        for &(id, _, _) in &self.quadtree.query(g.get_screen_bounds().as_bbox()) {
            objects.push(*id);
        }
        objects.sort_by_key(|id| id.zorder());

        for id in objects {
            g.redraw(&self.objects[&id].draw);
        }

        if let Some(id) = self.current_selection {
            let obj = &self.objects[&id];
            g.draw_polygon(Color::CYAN, &obj.polygon);
            g.draw_mouse_tooltip(&obj.info);
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
            if self.objects[&id].polygon.contains_pt(cursor) {
                self.current_selection = Some(id);
                return;
            }
        }
    }

    pub fn get_selection(&self) -> Option<ID> {
        self.current_selection
    }

    // TODO This and delete_obj assume the original bounds passed to the quadtree are still valid.
    pub fn add_obj(
        &mut self,
        prerender: &Prerender,
        id: ID,
        polygon: Polygon,
        color: Color,
        info: Text,
    ) {
        let quadtree_id = self
            .quadtree
            .insert_with_box(id, polygon.get_bounds().as_bbox());
        let draw = prerender.upload_borrowed(vec![(color, &polygon)]);
        self.objects.insert(
            id,
            Object {
                polygon,
                draw,
                info,
                quadtree_id,
            },
        );
    }

    pub fn delete_obj(&mut self, id: ID) {
        let obj = self.objects.remove(&id).unwrap();
        self.quadtree.remove(obj.quadtree_id).unwrap();
    }
}
