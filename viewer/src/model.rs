use aabb_quadtree::QuadTree;
use abstutil::{read_binary, Timer};
use ezgui::{Color, Drawable, EventCtx, GfxCtx, Prerender, Text};
use geom::{Circle, Distance, Polygon};
use map_model::raw_data;
use map_model::raw_data::{StableIntersectionID, StableRoadID};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum ID {
    // Forwards?
    HalfRoad(StableRoadID, bool),
    Intersection(StableIntersectionID),
}

impl ID {
    // Higher shows up in the front
    fn zorder(&self) -> usize {
        match self {
            ID::HalfRoad(_, _) => 0,
            ID::Intersection(_) => 1,
        }
    }
}

struct Object {
    polygon: Polygon,
    draw: Drawable,
    info: Text,
}

pub struct World {
    pub name: String,
    objects: HashMap<ID, Object>,
    quadtree: QuadTree<ID>,
}

impl World {
    pub fn load_initial_map(filename: &str, prerender: &Prerender) -> World {
        let data: raw_data::InitialMap =
            read_binary(filename, &mut Timer::new("load data")).unwrap();

        let mut w = World {
            name: filename.to_string(),
            objects: HashMap::new(),
            quadtree: QuadTree::default(data.bounds.as_bbox()),
        };

        for r in data.roads.values() {
            if r.fwd_width > Distance::ZERO {
                w.add_obj(
                    prerender,
                    ID::HalfRoad(r.id, true),
                    r.trimmed_center_pts
                        .shift_right(r.fwd_width / 2.0)
                        .make_polygons(r.fwd_width),
                    Color::grey(0.8),
                    Text::from_line(format!("{} forwards", r.id)),
                );
            }
            if r.back_width > Distance::ZERO {
                w.add_obj(
                    prerender,
                    ID::HalfRoad(r.id, false),
                    r.trimmed_center_pts
                        .shift_left(r.back_width / 2.0)
                        .make_polygons(r.back_width),
                    Color::grey(0.6),
                    Text::from_line(format!("{} backwards", r.id)),
                );
            }
        }

        for i in data.intersections.values() {
            w.add_obj(
                prerender,
                ID::Intersection(i.id),
                Polygon::new(&i.polygon),
                Color::RED,
                Text::from_line(format!("{}", i.id)),
            );
        }

        w
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
    }

    pub fn draw_selected(&self, g: &mut GfxCtx, id: ID) {
        let obj = &self.objects[&id];
        g.draw_polygon(Color::BLUE, &obj.polygon);
        g.draw_text_at(obj.info.clone(), obj.polygon.center());
    }

    pub fn mouseover_something(&self, ctx: &EventCtx) -> Option<ID> {
        let cursor = ctx.canvas.get_cursor_in_map_space()?;

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
                return Some(id);
            }
        }
        None
    }

    fn add_obj(
        &mut self,
        prerender: &Prerender,
        id: ID,
        polygon: Polygon,
        color: Color,
        info: Text,
    ) {
        self.quadtree
            .insert_with_box(id, polygon.get_bounds().as_bbox());
        let draw = prerender.upload_borrowed(vec![(color, &polygon)]);
        self.objects.insert(
            id,
            Object {
                polygon,
                draw,
                info,
            },
        );
    }
}
