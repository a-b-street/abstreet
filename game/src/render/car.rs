use crate::helpers::ID;
use crate::render::{DrawCtx, DrawOptions, Renderable, OUTLINE_THICKNESS};
use ezgui::{Canvas, Color, Drawable, GeomBatch, GfxCtx, Line, Prerender, Text};
use geom::{Distance, PolyLine, Polygon, Pt2D};
use map_model::Map;
use sim::{CarID, DrawCarInput, VehicleType};

const CAR_WIDTH: Distance = Distance::const_meters(1.75);

pub struct DrawCar {
    pub id: CarID,
    body: PolyLine,
    body_polygon: Polygon,
    zorder: isize,
    label: Option<Text>,

    draw_default: Drawable,
}

impl DrawCar {
    pub fn new(input: DrawCarInput, map: &Map, prerender: &Prerender, canvas: &Canvas) -> DrawCar {
        let body_polygon = input.body.make_polygons(CAR_WIDTH);
        let mut draw_default = GeomBatch::new();
        if let Some(p) = input.body.make_polygons_with_uv(CAR_WIDTH) {
            draw_default.push(
                canvas.texture(match input.id.1 {
                    VehicleType::Car => "assets/agents/car.png",
                    VehicleType::Bike => "assets/agents/bike.png",
                    VehicleType::Bus => "assets/agents/bus.png",
                }),
                p,
            );
        } else {
            draw_default.push(Color::CYAN, body_polygon.clone());
        }

        DrawCar {
            id: input.id,
            body: input.body,
            body_polygon,
            zorder: input.on.get_zorder(map),
            label: input
                .label
                .map(|line| Text::from(Line(line).fg(Color::RED).size(20)).no_bg()),
            draw_default: prerender.upload(draw_default),
        }
    }
}

impl Renderable for DrawCar {
    fn get_id(&self) -> ID {
        ID::Car(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: &DrawOptions, _: &DrawCtx) {
        if let Some(color) = opts.color(self.get_id()) {
            g.draw_polygon(color, &self.body_polygon);
        } else {
            g.redraw(&self.draw_default);
        }

        if let Some(ref txt) = self.label {
            // TODO Would rotation make any sense? Or at least adjust position/size while turning.
            // Buses are a constant length, so hardcoding this is fine.
            g.draw_text_at_mapspace(txt, self.body.dist_along(Distance::meters(9.0)).0);
        }
    }

    fn get_outline(&self, _: &Map) -> Polygon {
        self.body
            .to_thick_boundary(CAR_WIDTH, OUTLINE_THICKNESS)
            .unwrap_or_else(|| self.body_polygon.clone())
    }

    fn contains_pt(&self, pt: Pt2D, _: &Map) -> bool {
        self.body_polygon.contains_pt(pt)
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}
