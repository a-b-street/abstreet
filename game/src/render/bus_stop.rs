use crate::app::App;
use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::render::{DrawOptions, Renderable, OUTLINE_THICKNESS};
use ezgui::{Drawable, EventCtx, GeomBatch, GfxCtx};
use geom::{Angle, Circle, Distance, Line, Polygon, Pt2D};
use map_model::{BusStop, BusStopID, Map};

const RADIUS: Distance = Distance::const_meters(1.0);

pub struct DrawBusStop {
    pub id: BusStopID,
    center: Pt2D,
    zorder: isize,

    draw_default: Drawable,
}

impl DrawBusStop {
    pub fn new(ctx: &EventCtx, stop: &BusStop, map: &Map, cs: &ColorScheme) -> DrawBusStop {
        let (pt, angle) = stop.sidewalk_pos.pt_and_angle(map);
        let center = pt.project_away(
            map.get_l(stop.sidewalk_pos.lane()).width / 2.0,
            angle.rotate_degs(90.0),
        );

        let mut icon = GeomBatch::new();
        icon.append(
            GeomBatch::load_svg(
                ctx.prerender,
                if stop.is_train_stop {
                    "system/assets/map/light_rail.svg"
                } else {
                    "system/assets/meters/bus.svg"
                },
            )
            .scale(0.05)
            .centered_on(center),
        );
        let mut batch = GeomBatch::new();
        batch.push(
            cs.bus_layer.alpha(0.8),
            Circle::new(center, RADIUS).to_polygon(),
        );
        batch.append(icon.autocrop().centered_on(center));
        batch.push(
            cs.stop_sign_pole,
            Line::new(
                center.project_away(RADIUS, Angle::new_degs(90.0)),
                center.project_away(1.5 * RADIUS, Angle::new_degs(90.0)),
            )
            .unwrap()
            .make_polygons(Distance::meters(0.3)),
        );

        DrawBusStop {
            id: stop.id,
            center,
            zorder: map.get_parent(stop.sidewalk_pos.lane()).zorder,
            draw_default: ctx.upload(batch),
        }
    }
}

impl Renderable for DrawBusStop {
    fn get_id(&self) -> ID {
        ID::BusStop(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, _: &App, _: &DrawOptions) {
        g.redraw(&self.draw_default);
    }

    fn get_outline(&self, _: &Map) -> Polygon {
        Circle::outline(self.center, RADIUS, OUTLINE_THICKNESS).expect("constants defined wrong")
    }

    fn contains_pt(&self, pt: Pt2D, _: &Map) -> bool {
        Circle::new(self.center, RADIUS).contains_pt(pt)
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}
