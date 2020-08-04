use crate::app::App;
use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::render::{DrawOptions, Renderable};
use ezgui::{Drawable, EventCtx, GeomBatch, GfxCtx, Line, Text};
use geom::{Distance, Polygon, Pt2D};
use map_model::{Map, Road, RoadID};
use std::cell::RefCell;

pub struct DrawRoad {
    pub id: RoadID,
    zorder: isize,

    draw_center_line: Drawable,
    label: RefCell<Option<Drawable>>,
}

impl DrawRoad {
    pub fn new(ctx: &EventCtx, r: &Road, map: &Map, cs: &ColorScheme) -> DrawRoad {
        let mut draw = GeomBatch::new();

        // Only draw a center line if it straddles two driving/bike/bus lanes of opposite
        // directions
        if let (Some((_, lt1)), Some((_, lt2))) =
            (r.children_forwards.get(0), r.children_backwards.get(0))
        {
            if lt1.is_for_moving_vehicles() && lt2.is_for_moving_vehicles() {
                let width = Distance::meters(0.25);
                let color = if r.is_private() {
                    cs.road_center_line.lerp(cs.private_road, 0.5)
                } else {
                    cs.road_center_line
                };
                draw.extend(
                    color,
                    r.get_current_center(map).dashed_lines(
                        width,
                        Distance::meters(2.0),
                        Distance::meters(1.0),
                    ),
                );
            }
        }

        DrawRoad {
            id: r.id,
            zorder: r.zorder,
            draw_center_line: ctx.upload(draw),
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
                        let (pt, angle) = r.center_pts.must_dist_along(r.center_pts.length() / 2.0);
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
