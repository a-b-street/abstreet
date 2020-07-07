use crate::app::App;
use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::render::{DrawOptions, Renderable};
use ezgui::{Drawable, GeomBatch, GfxCtx, Line, Prerender, Text};
use geom::{Distance, Polygon, Pt2D};
use map_model::{LaneType, Map, Road, RoadID};
use std::cell::RefCell;

pub struct DrawRoad {
    pub id: RoadID,
    zorder: isize,

    draw_center_line: Drawable,
    label: RefCell<Option<Drawable>>,
}

impl DrawRoad {
    pub fn new(r: &Road, map: &Map, cs: &ColorScheme, prerender: &Prerender) -> DrawRoad {
        let mut draw = GeomBatch::new();
        let center = r.get_current_center(map);
        let width = Distance::meters(0.25);
        let color = if r.is_private() {
            cs.road_center_line.lerp(cs.private_road, 0.5)
        } else {
            cs.road_center_line
        };
        // If the road is a one-way (only parking and sidewalk on the off-side), draw a solid line
        // No center line at all if there's a shared left turn lane or it's light rail
        if !r.is_light_rail()
            && r.children_backwards
                .iter()
                .all(|(_, lt)| *lt == LaneType::Parking || *lt == LaneType::Sidewalk)
        {
            draw.push(color, center.make_polygons(width));
        } else if r.children_forwards.is_empty()
            || (r.children_forwards[0].1 != LaneType::SharedLeftTurn && !r.is_light_rail())
        {
            draw.extend(
                color,
                center.dashed_lines(width, Distance::meters(2.0), Distance::meters(1.0)),
            );
        }

        DrawRoad {
            id: r.id,
            zorder: r.zorder,
            draw_center_line: prerender.upload(draw),
            label: RefCell::new(None),
        }
    }
}

impl Renderable for DrawRoad {
    fn get_id(&self) -> ID {
        ID::Road(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, app: &App, _: &DrawOptions) {
        g.redraw(&self.draw_center_line);

        if app.opts.label_roads {
            // Lazily calculate
            let mut label = self.label.borrow_mut();
            if label.is_none() {
                let mut batch = GeomBatch::new();
                let r = app.primary.map.get_r(self.id);
                if !r.is_light_rail() {
                    let name = r.get_name();
                    if r.center_pts.length() >= Distance::meters(30.0) && name != "???" {
                        // TODO If it's definitely straddling bus/bike lanes, change the color? Or
                        // even easier, just skip the center lines?
                        let fg = if r.is_private() {
                            app.cs.road_center_line.lerp(app.cs.private_road, 0.5)
                        } else {
                            app.cs.road_center_line
                        };
                        let bg = if r.is_private() {
                            app.cs.driving_lane.lerp(app.cs.private_road, 0.5)
                        } else {
                            app.cs.driving_lane
                        };
                        let txt = Text::from(Line(name).fg(fg)).bg(bg);
                        let (pt, angle) = r.center_pts.dist_along(r.center_pts.length() / 2.0);
                        batch.append(
                            txt.render_to_batch(g.prerender)
                                .scale(0.1)
                                .centered_on(pt)
                                .rotate(angle.reorient()),
                        );
                    }
                }
                *label = Some(g.prerender.upload(batch));
            }
            // TODO Covered up sometimes. We could fork and force a different z value...
            g.redraw(label.as_ref().unwrap());
        }
    }

    fn get_outline(&self, map: &Map) -> Polygon {
        // Highlight the entire thing, not just an outline
        map.get_r(self.id).get_thick_polygon(map)
    }

    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        map.get_r(self.id).get_thick_polygon(map).contains_pt(pt)
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}
