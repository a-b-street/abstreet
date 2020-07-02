use crate::app::App;
use crate::common::heatmap::Grid;
use crate::game::{State, Transition};
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, VerticalAlignment, Widget,
};
use geom::{Duration, Polygon, Pt2D};
use map_model::{connectivity, IntersectionID};

// TODO Move cursor live
pub struct IsochroneViewer {
    composite: Composite,
    draw: Drawable,
}

impl IsochroneViewer {
    pub fn new(ctx: &mut EventCtx, app: &App, start: IntersectionID) -> Box<dyn State> {
        let draw = make_isochrone(ctx, app, start);
        Box::new(IsochroneViewer {
            composite: Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        Line("Isochrone").small_heading().draw(ctx),
                        Btn::text_fg("X")
                            .build(ctx, "close", hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    // TODO legend, mode picker
                ])
                .padding(16)
                .bg(app.cs.panel_bg),
            )
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

// TODO Probably to start, just color lanes/intersections
fn make_isochrone(ctx: &mut EventCtx, app: &App, start: IntersectionID) -> Drawable {
    let bounds = app.primary.map.get_bounds();
    let resolution_m = 100.0;
    let mut grid: Grid<Duration> = Grid::new(
        (bounds.width() / resolution_m).ceil() as usize,
        (bounds.height() / resolution_m).ceil() as usize,
        Duration::ZERO,
    );

    for (i, cost) in connectivity::all_costs_from(&app.primary.map, start) {
        let pt = app.primary.map.get_i(i).polygon.center();
        let idx = grid.idx(
            ((pt.x() - bounds.min_x) / resolution_m) as usize,
            ((pt.y() - bounds.min_y) / resolution_m) as usize,
        );
        // Don't add! If two intersections map to the same cell, should pick a finer resolution.
        grid.data[idx] = cost;
    }

    // Turn into contours
    let mut rows = Vec::new();
    for row in grid.data.chunks(grid.width) {
        rows.push(row.into_iter().map(|x| x.inner_seconds() as i16).collect());
    }
    let field = marching_squares::Field {
        dimensions: (grid.width, grid.height),
        top_left: marching_squares::Point { x: 0.0, y: 0.0 },
        pixel_size: (resolution_m as f32, resolution_m as f32),
        values: &rows,
    };
    let mut batch = GeomBatch::new();
    for (color, threshold) in vec![
        (Color::RED.alpha(0.5), Duration::seconds(30.0)),
        (Color::BLUE.alpha(0.5), Duration::minutes(2)),
    ] {
        for line in field.get_contours(threshold.inner_seconds() as i16) {
            if line.points.len() >= 3 {
                batch.push(
                    color,
                    Polygon::new(
                        &line
                            .points
                            .into_iter()
                            .map(|pt| Pt2D::new(pt.x.into(), pt.y.into()))
                            .collect(),
                    ),
                );
            }
        }
    }

    batch.upload(ctx)
}
