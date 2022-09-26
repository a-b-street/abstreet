use std::collections::HashMap;

use geom::{Bounds, Duration, Histogram, Polygon, Pt2D, Statistic};
use map_model::{BuildingID, Map};
use widgetry::tools::{ColorLegend, ColorScale};
use widgetry::{
    Choice, Color, EventCtx, GeomBatch, Panel, RoundedF64, Spinner, TextExt, Toggle, Widget,
};

const NEIGHBORS: [[isize; 2]; 9] = [
    [0, 0],
    [-1, 1],
    [-1, 0],
    [-1, -1],
    [0, -1],
    [1, -1],
    [1, 0],
    [1, 1],
    [0, 1],
];

#[derive(Clone, PartialEq)]
pub struct HeatmapOptions {
    // In meters
    resolution: f64,
    radius: f64,
    smoothing: bool,
    contours: bool,
    color_scheme: String,
}

impl HeatmapOptions {
    pub fn new() -> HeatmapOptions {
        HeatmapOptions {
            resolution: 10.0,
            radius: 3.0,
            smoothing: true,
            contours: true,
            color_scheme: "Turbo".to_string(),
        }
    }

    pub fn to_controls(&self, ctx: &mut EventCtx, legend: Widget) -> Vec<Widget> {
        vec![
            // TODO Display the value...
            Widget::row(vec![
                "Resolution (meters)".text_widget(ctx).centered_vert(),
                Spinner::f64_widget(ctx, "resolution", (1.0, 100.0), self.resolution, 1.0)
                    .align_right(),
            ]),
            Widget::row(vec![
                "Radius (resolution multiplier)"
                    .text_widget(ctx)
                    .centered_vert(),
                Spinner::f64_widget(ctx, "radius", (0.0, 10.0), self.radius, 1.0).align_right(),
            ]),
            Toggle::switch(ctx, "smoothing", None, self.smoothing),
            Toggle::switch(ctx, "contours", None, self.contours),
            Widget::row(vec![
                "Color scheme".text_widget(ctx).centered_vert(),
                Widget::dropdown(
                    ctx,
                    "Color scheme",
                    self.color_scheme.clone(),
                    vec!["Turbo", "Inferno", "Warm", "Cool", "Oranges", "Spectral"]
                        .into_iter()
                        .map(Choice::string)
                        .collect(),
                ),
            ]),
            legend,
        ]
    }

    pub fn from_controls(c: &Panel) -> HeatmapOptions {
        // Did we just change?
        if c.has_widget("resolution") {
            HeatmapOptions {
                resolution: c.spinner::<RoundedF64>("resolution").0,
                radius: c.spinner::<RoundedF64>("radius").0,
                smoothing: c.is_checked("smoothing"),
                contours: c.is_checked("contours"),
                color_scheme: c.dropdown_value("Color scheme"),
            }
        } else {
            HeatmapOptions::new()
        }
    }
}

// Returns a legend
pub fn make_heatmap(
    ctx: &mut EventCtx,
    batch: &mut GeomBatch,
    bounds: &Bounds,
    pts: Vec<Pt2D>,
    opts: &HeatmapOptions,
) -> Widget {
    // 7 colors, 8 labels
    let num_colors = 7;
    let gradient = match opts.color_scheme.as_ref() {
        "Turbo" => colorous::TURBO,
        "Inferno" => colorous::INFERNO,
        "Warm" => colorous::WARM,
        "Cool" => colorous::COOL,
        "Oranges" => colorous::ORANGES,
        "Spectral" => colorous::SPECTRAL,
        _ => unreachable!(),
    };
    let colors: Vec<Color> = (0..num_colors)
        .map(|i| {
            let c = gradient.eval_rational(i, num_colors);
            Color::rgb(c.r as usize, c.g as usize, c.b as usize)
        })
        .collect();

    if pts.is_empty() {
        let labels = std::iter::repeat("0".to_string())
            .take(num_colors + 1)
            .collect();
        return ColorLegend::gradient(ctx, &ColorScale(colors), labels);
    }

    // At each point, add a 2D Gaussian kernel centered at the point.
    let mut raw_grid: Grid<f64> = Grid::new(
        (bounds.width() / opts.resolution).ceil() as usize,
        (bounds.height() / opts.resolution).ceil() as usize,
        0.0,
    );
    for pt in pts {
        let base_x = ((pt.x() - bounds.min_x) / opts.resolution) as isize;
        let base_y = ((pt.y() - bounds.min_y) / opts.resolution) as isize;
        let denom = 2.0 * (opts.radius / 2.0).powi(2);

        let r = opts.radius as isize;
        for x in base_x - r..=base_x + r {
            for y in base_y - r..=base_y + r {
                let loc_r2 = (x - base_x).pow(2) + (y - base_y).pow(2);
                if x > 0
                    && y > 0
                    && x < (raw_grid.width as isize)
                    && y < (raw_grid.height as isize)
                    && loc_r2 <= r * r
                {
                    // https://en.wikipedia.org/wiki/Gaussian_function#Two-dimensional_Gaussian_function
                    let value = (-(((x - base_x) as f64).powi(2) / denom
                        + ((y - base_y) as f64).powi(2) / denom))
                        .exp();
                    let idx = raw_grid.idx(x as usize, y as usize);
                    raw_grid.data[idx] += value;
                }
            }
        }
    }

    let mut grid: Grid<f64> = Grid::new(
        (bounds.width() / opts.resolution).ceil() as usize,
        (bounds.height() / opts.resolution).ceil() as usize,
        0.0,
    );
    if opts.smoothing {
        for y in 0..raw_grid.height {
            for x in 0..raw_grid.width {
                let mut div = 1;
                let idx = grid.idx(x, y);
                grid.data[idx] = raw_grid.data[idx];
                for offset in &NEIGHBORS {
                    let next_x = x as isize + offset[0];
                    let next_y = y as isize + offset[1];
                    if next_x > 0
                        && next_y > 0
                        && next_x < (raw_grid.width as isize)
                        && next_y < (raw_grid.height as isize)
                    {
                        div += 1;
                        let next_idx = grid.idx(next_x as usize, next_y as usize);
                        grid.data[idx] += raw_grid.data[next_idx];
                    }
                }
                grid.data[idx] /= div as f64;
            }
        }
    } else {
        grid = raw_grid;
    }

    let mut distrib = Histogram::new();
    for count in &grid.data {
        // TODO Just truncate the decimal?
        distrib.add(*count as usize);
    }

    if opts.contours {
        let max = distrib.select(Statistic::Max).unwrap() as f64;
        let mut thresholds: Vec<f64> = (0..=5).map(|i| (i as f64) / 5.0 * max).collect();
        // Skip 0; it'll cover the entire map. But have a low value to distinguish
        // nothing/something.
        thresholds[0] = 0.1;
        let contour_builder = contour::ContourBuilder::new(grid.width as u32, grid.height as u32, false);
        for contour in contour_builder.contours(&grid.data, &thresholds).unwrap() {
            let (geometry, threshold) = contour.into_inner();

            let c = gradient.eval_continuous(threshold / max);
            // Don't block the map underneath
            let color = Color::rgb(c.r as usize, c.g as usize, c.b as usize).alpha(0.6);

            for geo_poly in geometry {
                if let Ok(poly) = Polygon::try_from(geo_poly) {
                    batch.push(color, poly.must_scale(opts.resolution));
                }
            }
        }
    } else {
        // Now draw rectangles
        let square = Polygon::rectangle(opts.resolution, opts.resolution);
        for y in 0..grid.height {
            for x in 0..grid.width {
                let count = grid.data[grid.idx(x, y)];
                if count > 0.0 {
                    let pct = (count as f64) / (distrib.select(Statistic::Max).unwrap() as f64);
                    let c = gradient.eval_continuous(pct);
                    // Don't block the map underneath
                    let color = Color::rgb(c.r as usize, c.g as usize, c.b as usize).alpha(0.6);
                    batch.push(
                        color,
                        square
                            .translate((x as f64) * opts.resolution, (y as f64) * opts.resolution),
                    );
                }
            }
        }
    }

    let mut labels = vec!["0".to_string()];
    for i in 1..=num_colors {
        let pct = (i as f64) / (num_colors as f64);
        labels.push(
            (pct * (distrib.select(Statistic::Max).unwrap() as f64))
                .round()
                .to_string(),
        );
    }
    ColorLegend::gradient(ctx, &ColorScale(colors), labels)
}

/// A 2D grid containing some arbitrary data.
pub struct Grid<T> {
    /// Logically represents a 2D vector. Row-major ordering.
    pub data: Vec<T>,
    pub width: usize,
    pub height: usize,
}

impl<T: Copy> Grid<T> {
    pub fn new(width: usize, height: usize, default: T) -> Grid<T> {
        Grid {
            data: std::iter::repeat(default).take(width * height).collect(),
            width,
            height,
        }
    }

    /// Calculate the index from a given (x, y). Doesn't do any bounds checking.
    pub fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    /// The inverse of `idx`. No bounds checking.
    pub fn xy(&self, idx: usize) -> (usize, usize) {
        let y = idx / self.width;
        let x = idx % self.width;
        (x, y)
    }

    /// From one tile, calculate the 4 orthogonal neighbors. Includes bounds checking.
    pub fn orthogonal_neighbors(&self, center_x: usize, center_y: usize) -> Vec<(usize, usize)> {
        let center_x = center_x as isize;
        let center_y = center_y as isize;
        let mut results = Vec::new();
        for (dx, dy) in [(-1, 0), (0, -1), (0, 1), (1, 0)] {
            let x = center_x + dx;
            let y = center_y + dy;
            if x < 0 || (x as usize) >= self.width || y < 0 || (y as usize) >= self.height {
                continue;
            }
            results.push((x as usize, y as usize));
        }
        results
    }
}

// TODO Refactor the variations of this.
/// Thresholds are Durations, in units of seconds
pub fn draw_isochrone(
    map: &Map,
    time_to_reach_building: &HashMap<BuildingID, Duration>,
    thresholds: &[f64],
    colors: &[Color],
) -> GeomBatch {
    // To generate the polygons covering areas between 0-5 mins, 5-10 mins, etc, we have to feed
    // in a 2D grid of costs. Use a 100x100 meter resolution.
    let bounds = map.get_bounds();
    let resolution_m = 100.0;
    // The costs we're storing are currently durations, but the contour crate needs f64, so
    // just store the number of seconds.
    let mut grid: Grid<f64> = Grid::new(
        (bounds.width() / resolution_m).ceil() as usize,
        (bounds.height() / resolution_m).ceil() as usize,
        0.0,
    );

    for (b, cost) in time_to_reach_building {
        // What grid cell does the building belong to?
        let pt = map.get_b(*b).polygon.center();
        let idx = grid.idx(
            ((pt.x() - bounds.min_x) / resolution_m) as usize,
            ((pt.y() - bounds.min_y) / resolution_m) as usize,
        );
        // Don't add! If two buildings map to the same cell, we should pick a finer resolution.
        grid.data[idx] = cost.inner_seconds();
    }

    let smooth = false;
    let contour_builder = contour::ContourBuilder::new(grid.width as u32, grid.height as u32, smooth);
    let mut batch = GeomBatch::new();
    // The last feature returned will be larger than the last threshold value. We don't want to
    // display that at all. zip() will omit this last pair, since colors.len() ==
    // thresholds.len() - 1.
    //
    // TODO Actually, this still isn't working. I think each polygon is everything > the
    // threshold, not everything between two thresholds?
    for (contour, color) in contour_builder
        .contours(&grid.data, thresholds)
        .unwrap()
        .into_iter()
        .zip(colors)
    {
        let (polygons, _) = contour.into_inner();
        for p in polygons {
            if let Ok(poly) = Polygon::try_from(p) {
                batch.push(*color, poly.must_scale(resolution_m));
            }
        }
    }

    batch
}
