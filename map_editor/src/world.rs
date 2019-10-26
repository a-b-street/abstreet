use aabb_quadtree::{ItemId, QuadTree};
use ezgui::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, Prerender, Text};
use geom::{Bounds, Circle, Distance, Polygon};
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

pub trait ObjectID: Clone + Copy + Debug + Eq + Hash {
    // Higher shows up in the front.
    fn zorder(&self) -> usize;
}

pub struct Object<ID: ObjectID> {
    id: ID,
    geometry: Vec<(Color, Polygon)>,
    label: Option<Text>,
}

impl<ID: ObjectID> Object<ID> {
    pub fn new(id: ID, color: Color, poly: Polygon) -> Object<ID> {
        Object {
            id,
            geometry: vec![(color, poly)],
            label: None,
        }
    }

    pub fn blank(id: ID) -> Object<ID> {
        Object {
            id,
            geometry: Vec::new(),
            label: None,
        }
    }

    pub fn get_id(&self) -> ID {
        self.id
    }

    pub fn push(&mut self, color: Color, poly: Polygon) {
        self.geometry.push((color, poly));
    }

    pub fn maybe_label(mut self, label: Option<String>) -> Object<ID> {
        assert!(self.label.is_none());
        if let Some(s) = label {
            self.label = Some(Text::from(Line(s)));
        }
        self
    }
}

struct WorldObject {
    unioned_polygon: Polygon,
    draw: Drawable,
    label: Option<Text>,
    quadtree_id: ItemId,
}

pub struct World<ID: ObjectID> {
    objects: HashMap<ID, WorldObject>,
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
            if let Some(ref txt) = obj.label {
                g.draw_text_at(txt, obj.unioned_polygon.center());
            }
        }

        if let Some(id) = self.current_selection {
            let obj = &self.objects[&id];
            g.draw_polygon(Color::CYAN.alpha(0.5), &obj.unioned_polygon);
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
    pub fn add(&mut self, prerender: &Prerender, obj: Object<ID>) {
        let mut unioned_polygon = obj.geometry[0].1.clone();
        for (_, p) in &obj.geometry[1..] {
            unioned_polygon = unioned_polygon.union(p.clone());
        }

        let quadtree_id = self
            .quadtree
            .insert_with_box(obj.id, unioned_polygon.get_bounds().as_bbox());
        let draw = prerender.upload(GeomBatch::from(obj.geometry));
        self.objects.insert(
            obj.id,
            WorldObject {
                unioned_polygon,
                draw,
                quadtree_id,
                label: obj.label,
            },
        );
    }

    pub fn delete(&mut self, id: ID) {
        let obj = self.objects.remove(&id).unwrap();
        self.quadtree.remove(obj.quadtree_id).unwrap();
    }

    pub fn get_unioned_polygon(&self, id: ID) -> Option<&Polygon> {
        Some(&self.objects.get(&id)?.unioned_polygon)
    }
}
