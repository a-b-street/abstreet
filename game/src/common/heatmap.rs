use ezgui::{Choice, Color, GeomBatch};
use geom::{Bounds, Histogram, Polygon, Pt2D};

#[derive(Clone, PartialEq)]
pub struct HeatmapOptions {
    // In meters
    pub resolution: usize,
    pub radius: usize,
    pub colors: HeatmapColors,
}

impl HeatmapOptions {
    pub fn new() -> HeatmapOptions {
        HeatmapOptions {
            resolution: 10,
            radius: 3,
            colors: HeatmapColors::FullSpectral,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum HeatmapColors {
    FullSpectral,
    SingleHue,
}

impl HeatmapColors {
    pub fn choices() -> Vec<Choice<HeatmapColors>> {
        vec![
            Choice::new("full spectral", HeatmapColors::FullSpectral),
            Choice::new("single hue", HeatmapColors::SingleHue),
        ]
    }
}

// Returns the ordered list of (max value per bucket, color)
pub fn make_heatmap(
    batch: &mut GeomBatch,
    bounds: &Bounds,
    pts: Vec<Pt2D>,
    opts: &HeatmapOptions,
) -> Vec<(f64, Color)> {
    if pts.is_empty() {
        return Vec::new();
    }

    let mut grid: Grid<f64> = Grid::new(
        (bounds.width() / opts.resolution as f64).ceil() as usize,
        (bounds.height() / opts.resolution as f64).ceil() as usize,
        0.0,
    );

    // At each point, add a 2D Gaussian kernel centered at the point.
    for pt in pts {
        let base_x = ((pt.x() - bounds.min_x) / opts.resolution as f64) as isize;
        let base_y = ((pt.y() - bounds.min_y) / opts.resolution as f64) as isize;
        let denom = 2.0 * (opts.radius as f64).powi(2);

        let r = opts.radius as isize;
        for x in base_x - r..=base_x + r {
            for y in base_y - r..=base_y + r {
                if x > 0 && y > 0 && x < (grid.width as isize) && y < (grid.height as isize) {
                    // https://en.wikipedia.org/wiki/Gaussian_function#Two-dimensional_Gaussian_function
                    // TODO Amplitude of 1 fine?
                    let value = (-(((x - base_x) as f64).powi(2) / denom
                        + ((y - base_y) as f64).powi(2) / denom))
                        .exp();
                    let idx = grid.idx(x as usize, y as usize);
                    grid.data[idx] += value;
                }
            }
        }
    }

    let mut distrib = Histogram::new();
    for count in &grid.data {
        // TODO Just truncate the decimal?
        distrib.add(*count as usize);
    }

    // This is in order from low density to high.
    let colors = match opts.colors {
        HeatmapColors::FullSpectral => vec![
            Color::hex("#0b2c7a"),
            Color::hex("#1e9094"),
            Color::hex("#0ec441"),
            Color::hex("#7bed00"),
            Color::hex("#f7d707"),
            Color::hex("#e68e1c"),
            Color::hex("#c2523c"),
        ],
        HeatmapColors::SingleHue => vec![
            Color::hex("#FFEBD6"),
            Color::hex("#F5CBAE"),
            Color::hex("#EBA988"),
            Color::hex("#E08465"),
            Color::hex("#D65D45"),
            Color::hex("#CC3527"),
            Color::hex("#C40A0A"),
        ],
    };
    let num_colors = colors.len();
    let max_count_per_bucket: Vec<(f64, Color)> = (1..=num_colors)
        .map(|i| {
            distrib
                .percentile(100.0 * (i as f64) / (num_colors as f64))
                .unwrap() as f64
        })
        .zip(colors.into_iter())
        .collect();

    // Now draw rectangles
    let square = Polygon::rectangle(opts.resolution as f64, opts.resolution as f64);
    for y in 0..grid.height {
        for x in 0..grid.width {
            let idx = grid.idx(x, y);
            let count = grid.data[idx];
            if count > 0.0 {
                let mut color = max_count_per_bucket[0].1;
                for (max, c) in &max_count_per_bucket {
                    if count >= *max {
                        color = *c;
                    } else {
                        break;
                    }
                }

                batch.push(
                    color,
                    square.translate((x * opts.resolution) as f64, (y * opts.resolution) as f64),
                );
            }
        }
    }

    max_count_per_bucket
}

struct Grid<T> {
    data: Vec<T>,
    width: usize,
    height: usize,
}

impl<T: Copy> Grid<T> {
    fn new(width: usize, height: usize, default: T) -> Grid<T> {
        Grid {
            data: std::iter::repeat(default).take(width * height).collect(),
            width,
            height,
        }
    }

    fn idx(&self, x: usize, y: usize) -> usize {
        // Row-major
        y * self.width + x
    }
}
