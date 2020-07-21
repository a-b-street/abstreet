use crate::app::App;
use crate::common::heatmap::Grid;
use crate::game::{State, Transition};
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, VerticalAlignment, Widget,
};
use geom::{Distance, Polygon};
use map_model::{connectivity, BuildingID};

// TODO Move cursor live
pub struct IsochroneViewer {
    composite: Composite,
    draw: Drawable,
}

impl IsochroneViewer {
    pub fn new(ctx: &mut EventCtx, app: &App, start: BuildingID) -> Box<dyn State> {
        let draw = make_isochrone(ctx, app, start);
        Box::new(IsochroneViewer {
            composite: Composite::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Isochrone").small_heading().draw(ctx),
                    Btn::text_fg("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                // TODO legend, mode picker
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            draw,
        })
    }
}

impl State for IsochroneViewer {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.draw);
        self.composite.draw(g);
    }
}

fn make_isochrone(ctx: &mut EventCtx, app: &App, start: BuildingID) -> Drawable {
    let bounds = app.primary.map.get_bounds();
    let resolution_m = 100.0;
    // Distance in meters
    let mut grid: Grid<f64> = Grid::new(
        (bounds.width() / resolution_m).ceil() as usize,
        (bounds.height() / resolution_m).ceil() as usize,
        0.0,
    );

    for (b, cost) in connectivity::all_costs_from(&app.primary.map, start) {
        let pt = app.primary.map.get_b(b).polygon.center();
        let idx = grid.idx(
            ((pt.x() - bounds.min_x) / resolution_m) as usize,
            ((pt.y() - bounds.min_y) / resolution_m) as usize,
        );
        // Don't add! If two buildings map to the same cell, should pick a finer resolution.
        grid.data[idx] = cost.inner_meters();
    }

    let thresholds = vec![
        0.1,
        Distance::miles(0.5).inner_meters(),
        Distance::miles(3.0).inner_meters(),
        Distance::miles(6.0).inner_meters(),
    ];
    let colors = vec![
        Color::BLACK.alpha(0.5),
        Color::GREEN.alpha(0.5),
        Color::BLUE.alpha(0.5),
        Color::RED.alpha(0.5),
    ];
    let c = contour::ContourBuilder::new(grid.width as u32, grid.height as u32, false);
    let mut batch = GeomBatch::new();
    for (feature, color) in c
        .contours(&grid.data, &thresholds)
        .unwrap()
        .into_iter()
        .zip(colors)
    {
        match feature.geometry.unwrap().value {
            geojson::Value::MultiPolygon(polygons) => {
                for p in polygons {
                    batch.push(color, Polygon::from_geojson(&p).scale(resolution_m));
                }
            }
            _ => unreachable!(),
        }
    }

    batch.upload(ctx)
}
