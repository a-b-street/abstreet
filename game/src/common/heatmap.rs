use geom::{Bounds, Histogram, Polygon, Pt2D, Statistic};
use map_gui::common::ColorScale;
use widgetry::{Checkbox, Choice, Color, EventCtx, GeomBatch, Panel, Spinner, TextExt, Widget};

use crate::common::ColorLegend;

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
    resolution: usize,
    radius: usize,
    smoothing: bool,
    contours: bool,
    color_scheme: String,
}

impl HeatmapOptions {
    pub fn new() -> HeatmapOptions {
        HeatmapOptions {
            resolution: 10,
            radius: 3,
            smoothing: true,
            contours: true,
            color_scheme: "Turbo".to_string(),
        }
    }

    pub fn to_controls(&self, ctx: &mut EventCtx, legend: Widget) -> Vec<Widget> {
        vec![
            // TODO Display the value...
            Widget::row(vec![
                "Resolution (meters)".draw_text(ctx).centered_vert(),
                Spinner::new(ctx, (1, 100), self.resolution as isize)
                    .named("resolution")
                    .align_right(),
            ]),
            Widget::row(vec![
                "Radius (resolution multiplier)"
                    .draw_text(ctx)
                    .centered_vert(),
                Spinner::new(ctx, (0, 10), self.radius as isize)
                    .named("radius")
                    .align_right(),
            ]),
            Checkbox::switch(ctx, "smoothing", None, self.smoothing),
            Checkbox::switch(ctx, "contours", None, self.contours),
            Widget::row(vec![
                "Color scheme".draw_text(ctx).centered_vert(),
                Widget::dropdown(
                    ctx,
                    "Color scheme",
                    self.color_scheme.clone(),
                    vec!["Turbo", "Inferno", "Warm", "Cool", "Oranges", "Spectral"]
                        .into_iter()
                        .map(|x| Choice::string(x))
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
                resolution: c.spinner("resolution") as usize,
                radius: c.spinner("radius") as usize,
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
        (bounds.width() / opts.resolution as f64).ceil() as usize,
        (bounds.height() / opts.resolution as f64).ceil() as usize,
        0.0,
    );
    for pt in pts {
        let base_x = ((pt.x() - bounds.min_x) / opts.resolution as f64) as isize;
        let base_y = ((pt.y() - bounds.min_y) / opts.resolution as f64) as isize;
        let denom = 2.0 * (opts.radius as f64 / 2.0).powi(2);

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
        (bounds.width() / opts.resolution as f64).ceil() as usize,
        (bounds.height() / opts.resolution as f64).ceil() as usize,
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
        let c = contour::ContourBuilder::new(grid.width as u32, grid.height as u32, false);
        for (feature, value) in c
            .contours(&grid.data, &thresholds)
            .unwrap()
            .into_iter()
            .zip(thresholds)
        {
            // TODO We should technically not depend on the order of features matching the input
            // thresholds, but extracting geojson values doesn't sound fun
            match feature.geometry.unwrap().value {
                geojson::Value::MultiPolygon(polygons) => {
                    let c = gradient.eval_continuous(value / max);
                    // Don't block the map underneath
                    let color = Color::rgb(c.r as usize, c.g as usize, c.b as usize).alpha(0.6);
                    for p in polygons {
                        batch.push(
                            color,
                            Polygon::from_geojson(&p).scale(opts.resolution as f64),
                        );
                    }
                }
                _ => unreachable!(),
            }
        }
    } else {
        // Now draw rectangles
        let square = Polygon::rectangle(opts.resolution as f64, opts.resolution as f64);
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
                            .translate((x * opts.resolution) as f64, (y * opts.resolution) as f64),
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

pub struct Grid<T> {
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

    pub fn idx(&self, x: usize, y: usize) -> usize {
        // Row-major
        y * self.width + x
    }
}
