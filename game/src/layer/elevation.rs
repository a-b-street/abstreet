use geom::{Angle, Distance, FindClosest, PolyLine, Polygon, Pt2D};
use map_gui::tools::{ColorDiscrete, ColorScale, Grid};
use map_gui::ID;
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, Panel, Text, TextExt, Widget};

use crate::app::App;
use crate::layer::{header, Layer, LayerOutcome, PANEL_PLACEMENT};

pub struct SteepStreets {
    tooltip: Option<Text>,
    unzoomed: Drawable,
    zoomed: Drawable,
    panel: Panel,
}

impl Layer for SteepStreets {
    fn name(&self) -> Option<&'static str> {
        Some("steep streets")
    }
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<LayerOutcome> {
        if ctx.redo_mouseover() {
            self.tooltip = None;
            if let Some(ID::Road(r)) = app.mouseover_unzoomed_roads_and_intersections(ctx) {
                self.tooltip = Some(Text::from(format!(
                    "{:.1}% incline",
                    app.primary.map.get_r(r).percent_incline.abs() * 100.0
                )));
            }
        }

        <dyn Layer>::simple_event(ctx, &mut self.panel)
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
        if let Some(ref txt) = self.tooltip {
            g.draw_mouse_tooltip(txt.clone());
        }
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.unzoomed);
    }
}

impl SteepStreets {
    pub fn new(ctx: &mut EventCtx, app: &App) -> SteepStreets {
        let mut colorer = ColorDiscrete::new(
            app,
            vec![
                // Colors and buckets from https://github.com/ITSLeeds/slopes
                ("0-3% (flat)", Color::hex("#296B07")),
                ("3-5%", Color::hex("#689A03")),
                ("5-8%", Color::hex("#EB9A04")),
                ("8-10%", Color::hex("#D30800")),
                ("10-20%", Color::hex("#980104")),
                (">20% (steep)", Color::hex("#680605")),
            ],
        );

        let arrow_len = Distance::meters(5.0);
        let thickness = Distance::meters(2.0);
        let mut steepest = 0.0_f64;
        let mut arrows = GeomBatch::new();
        for r in app.primary.map.all_roads() {
            let pct = r.percent_incline.abs();
            steepest = steepest.max(pct);

            let bucket = if pct < 0.03 {
                "0-3% (flat)"
            } else if pct < 0.05 {
                "3-5%"
            } else if pct < 0.08 {
                "5-8%"
            } else if pct < 0.1 {
                "8-10%"
            } else if pct < 0.2 {
                "10-20%"
            } else {
                ">20% (steep)"
            };
            colorer.add_r(r.id, bucket);

            // Draw arrows pointing uphill
            // TODO Draw V's, not arrows.
            // TODO Or try gradient colors.
            if pct < 0.03 {
                continue;
            }
            let mut pl = r.center_pts.clone();
            if r.percent_incline < 0.0 {
                pl = pl.reversed();
            }

            for (pt, angle) in pl.step_along(Distance::meters(15.0), arrow_len) {
                arrows.push(
                    Color::WHITE,
                    PolyLine::must_new(vec![
                        pt.project_away(arrow_len, angle.rotate_degs(-135.0)),
                        pt,
                        pt.project_away(arrow_len, angle.rotate_degs(135.0)),
                    ])
                    .make_polygons(thickness),
                );
            }
        }
        colorer.unzoomed.append(arrows);
        let (unzoomed, zoomed, legend) = colorer.build(ctx);

        let pt = Pt2D::new(0.0, 0.0);
        let panel_arrow = PolyLine::must_new(vec![
            pt.project_away(arrow_len, Angle::degrees(-135.0)),
            pt,
            pt.project_away(arrow_len, Angle::degrees(135.0)),
        ])
        .make_polygons(thickness)
        .scale(5.0);
        let panel = Panel::new_builder(Widget::col(vec![
            header(ctx, "Steep streets"),
            Widget::row(vec![
                GeomBatch::from(vec![(ctx.style().text_primary_color, panel_arrow)])
                    .autocrop()
                    .into_widget(ctx),
                "points uphill".text_widget(ctx).centered_vert(),
            ]),
            legend,
            format!("Steepest road: {:.0}% incline", steepest * 100.0).text_widget(ctx),
        ]))
        .aligned_pair(PANEL_PLACEMENT)
        .build(ctx);

        SteepStreets {
            tooltip: None,
            unzoomed,
            zoomed,
            panel,
        }
    }
}

const INTERSECTION_SEARCH_RADIUS: Distance = Distance::const_meters(300.0);
const CONTOUR_STEP_SIZE: Distance = Distance::const_meters(15.0);

pub struct ElevationContours {
    tooltip: Option<Text>,
    closest_elevation: FindClosest<Distance>,
    unzoomed: Drawable,
    panel: Panel,
}

impl Layer for ElevationContours {
    fn name(&self) -> Option<&'static str> {
        Some("elevation")
    }
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<LayerOutcome> {
        if ctx.redo_mouseover() {
            self.tooltip = None;
            if ctx.canvas.cam_zoom < app.opts.min_zoom_for_detail {
                if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                    if let Some((elevation, _)) = self
                        .closest_elevation
                        .closest_pt(pt, INTERSECTION_SEARCH_RADIUS)
                    {
                        self.tooltip = Some(Text::from(format!(
                            "Elevation: {}",
                            elevation.to_string(&app.opts.units)
                        )));
                    }
                }
            }
        }

        <dyn Layer>::simple_event(ctx, &mut self.panel)
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        }
        if let Some(ref txt) = self.tooltip {
            g.draw_mouse_tooltip(txt.clone());
        }
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.unzoomed);
    }
}

impl ElevationContours {
    pub fn new(ctx: &mut EventCtx, app: &App) -> ElevationContours {
        let mut low = Distance::ZERO;
        let mut high = Distance::ZERO;
        for i in app.primary.map.all_intersections() {
            low = low.min(i.elevation);
            high = high.max(i.elevation);
        }

        let (closest_elevation, unzoomed) = make_elevation_contours(ctx, app, low, high);

        let panel = Panel::new_builder(Widget::col(vec![
            header(ctx, "Elevation"),
            format!(
                "Elevation from {} to {}",
                low.to_string(&app.opts.units),
                high.to_string(&app.opts.units)
            )
            .text_widget(ctx),
        ]))
        .aligned_pair(PANEL_PLACEMENT)
        .build(ctx);

        ElevationContours {
            tooltip: None,
            closest_elevation,
            unzoomed,
            panel,
        }
    }
}

pub fn make_elevation_contours(
    ctx: &mut EventCtx,
    app: &App,
    low: Distance,
    high: Distance,
) -> (FindClosest<Distance>, Drawable) {
    let bounds = app.primary.map.get_bounds();
    let mut closest = FindClosest::new(bounds);
    let mut batch = GeomBatch::new();

    ctx.loading_screen("generate contours", |_, timer| {
        timer.start("gather input");

        let resolution_m = 30.0;
        // Elevation in meters
        let mut grid: Grid<f64> = Grid::new(
            (bounds.width() / resolution_m).ceil() as usize,
            (bounds.height() / resolution_m).ceil() as usize,
            0.0,
        );

        // Since gaps in the grid mess stuff up, just fill out each grid cell. Explicitly do the
        // interpolation to the nearest measurement we have.
        for i in app.primary.map.all_intersections() {
            // TODO Or maybe even just the center?
            closest.add(i.elevation, i.polygon.points());
        }
        let mut indices = Vec::new();
        for x in 0..grid.width {
            for y in 0..grid.height {
                indices.push((x, y));
            }
        }
        for (idx, elevation) in timer.parallelize("fill out grid", indices, |(x, y)| {
            let pt = Pt2D::new((x as f64) * resolution_m, (y as f64) * resolution_m);
            let elevation = match closest.closest_pt(pt, INTERSECTION_SEARCH_RADIUS) {
                Some((e, _)) => e,
                // No intersections nearby... assume ocean?
                None => Distance::ZERO,
            };
            (grid.idx(x, y), elevation)
        }) {
            grid.data[idx] = elevation.inner_meters();
        }
        timer.stop("gather input");

        timer.start("calculate contours");
        // Generate polygons covering the contour line where the cost in the grid crosses these
        // threshold values.
        let mut thresholds: Vec<f64> = Vec::new();
        let mut x = low;
        while x < high {
            thresholds.push(x.inner_meters());
            x += CONTOUR_STEP_SIZE;
        }
        // And color the polygon for each threshold
        let scale = ColorScale(vec![Color::WHITE, Color::RED]);
        let colors: Vec<Color> = (0..thresholds.len())
            .map(|i| scale.eval((i as f64) / (thresholds.len() as f64)))
            .collect();
        let smooth = false;
        let c = contour::ContourBuilder::new(grid.width as u32, grid.height as u32, smooth);
        let features = c.contours(&grid.data, &thresholds).unwrap();
        timer.stop("calculate contours");

        timer.start_iter("draw", features.len());
        for (feature, color) in features.into_iter().zip(colors) {
            timer.next();
            match feature.geometry.unwrap().value {
                geojson::Value::MultiPolygon(polygons) => {
                    for p in polygons {
                        if let Ok(p) = Polygon::from_geojson(&p) {
                            let poly = p.scale(resolution_m);
                            if let Ok(x) = poly.to_outline(Distance::meters(5.0)) {
                                batch.push(Color::BLACK.alpha(0.5), x);
                            }
                            batch.push(color.alpha(0.1), poly);
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
    });

    (closest, batch.upload(ctx))
}
