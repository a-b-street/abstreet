use crate::helpers::{ColorScheme, ID};
use crate::render::{DrawCtx, DrawOptions, Renderable, OUTLINE_THICKNESS};
use ezgui::{Canvas, Color, Drawable, GeomBatch, GfxCtx, Line, Prerender, Text};
use geom::{Circle, PolyLine, Polygon};
use map_model::{Map, LANE_THICKNESS};
use sim::{DrawPedCrowdInput, DrawPedestrianInput, PedCrowdLocation, PedestrianID};

pub struct DrawPedestrian {
    pub id: PedestrianID,
    body_circle: Circle,
    zorder: isize,

    draw_default: Drawable,
}

impl DrawPedestrian {
    pub fn new(
        input: DrawPedestrianInput,
        map: &Map,
        prerender: &Prerender,
        canvas: &Canvas,
    ) -> DrawPedestrian {
        let radius = LANE_THICKNESS / 4.0; // TODO make const after const fn is better
        let body_circle = Circle::new(input.pos, radius);
        let draw_default = GeomBatch::from(vec![(
            canvas
                .texture("assets/agents/pedestrian.png")
                .rotate(input.facing),
            body_circle.to_polygon(),
        )]);
        DrawPedestrian {
            id: input.id,
            body_circle,
            zorder: input.on.get_zorder(map),
            draw_default: prerender.upload(draw_default),
        }
    }
}

impl Renderable for DrawPedestrian {
    fn get_id(&self) -> ID {
        ID::Pedestrian(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: &DrawOptions, _: &DrawCtx) {
        if let Some(color) = opts.color(self.get_id()) {
            g.draw_circle(color, &self.body_circle);
        } else {
            g.redraw(&self.draw_default);
        }
    }

    fn get_outline(&self, _: &Map) -> Polygon {
        // TODO thin ring
        self.body_circle.to_polygon()
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}

pub struct DrawPedCrowd {
    members: Vec<PedestrianID>,
    blob: Polygon,
    blob_pl: PolyLine,
    zorder: isize,

    draw_default: Drawable,
    label: Text,
}

impl DrawPedCrowd {
    pub fn new(
        input: DrawPedCrowdInput,
        map: &Map,
        prerender: &Prerender,
        cs: &ColorScheme,
    ) -> DrawPedCrowd {
        let pl_shifted = match input.location {
            PedCrowdLocation::Sidewalk(on, contraflow) => {
                let pl_slice = on.exact_slice(input.low, input.high, map);
                if contraflow {
                    pl_slice.shift_left(LANE_THICKNESS / 4.0).unwrap()
                } else {
                    pl_slice.shift_right(LANE_THICKNESS / 4.0).unwrap()
                }
            }
            PedCrowdLocation::FrontPath(b) => map
                .get_b(b)
                .front_path
                .line
                .to_polyline()
                .exact_slice(input.low, input.high),
        };
        let blob = pl_shifted.make_polygons(LANE_THICKNESS / 2.0);
        let draw_default = prerender.upload_borrowed(vec![(cs.get("pedestrian"), &blob)]);

        // Ideally "pedestrian head" color, but it looks really faded...
        let label = Text::from(
            Line(format!("{}", input.members.len()))
                .fg(Color::BLACK)
                .size(15),
        )
        .no_bg();

        DrawPedCrowd {
            members: input.members,
            blob_pl: pl_shifted,
            blob,
            zorder: match input.location {
                PedCrowdLocation::Sidewalk(on, _) => on.get_zorder(map),
                PedCrowdLocation::FrontPath(_) => 0,
            },
            draw_default,
            label,
        }
    }
}

impl Renderable for DrawPedCrowd {
    fn get_id(&self) -> ID {
        // Expensive! :(
        ID::PedCrowd(self.members.clone())
    }

    fn draw(&self, g: &mut GfxCtx, opts: &DrawOptions, _: &DrawCtx) {
        if let Some(color) = opts.color(self.get_id()) {
            g.draw_polygon(color, &self.blob);
        } else {
            g.redraw(&self.draw_default);
        }
        g.draw_text_at_mapspace(&self.label, self.blob.center());
    }

    fn get_outline(&self, _: &Map) -> Polygon {
        self.blob_pl
            .to_thick_boundary(LANE_THICKNESS / 2.0, OUTLINE_THICKNESS)
            .unwrap_or_else(|| self.blob.clone())
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}
