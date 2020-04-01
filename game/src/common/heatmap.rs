use ezgui::{Choice, Color, GeomBatch};
use geom::{Bounds, Histogram, Polygon, Pt2D};

#[derive(Clone, PartialEq)]
pub struct HeatmapOptions {
    // In meters
    pub resolution: f64,
    pub num_passes: usize,
    pub colors: HeatmapColors,
}

impl HeatmapOptions {
    pub fn new() -> HeatmapOptions {
        HeatmapOptions {
            resolution: 10.0,
            num_passes: 5,
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

pub fn make_heatmap(batch: &mut GeomBatch, bounds: &Bounds, pts: Vec<Pt2D>, opts: &HeatmapOptions) {
    if pts.is_empty() {
        return;
    }

    // u8 is not quite enough -- one building could totally have more than 256 people.
    let mut counts: Grid<u16> = Grid::new(
        (bounds.width() / opts.resolution).ceil() as usize,
        (bounds.height() / opts.resolution).ceil() as usize,
        0,
    );

    for pt in pts {
        // TODO more careful rounding
        let idx = counts.idx(
            ((pt.x() - bounds.min_x) / opts.resolution) as usize,
            ((pt.y() - bounds.min_y) / opts.resolution) as usize,
        );
        counts.data[idx] += 1;
    }

    // Diffusion
    for _ in 0..opts.num_passes {
        // Have to hot-swap! Urgh
        let mut copy = counts.data.clone();
        for y in 0..counts.height {
            for x in 0..counts.width {
                let idx = counts.idx(x, y);
                if counts.data[idx] > 0 {
                    copy[idx] += 1;
                    for idx in counts.neighbors_8(x, y) {
                        copy[idx] += 1;
                    }
                }
            }
        }
        counts.data = copy;
    }

    let mut cnt_distrib = Histogram::new();
    for cnt in &counts.data {
        cnt_distrib.add(*cnt);
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
    let max_cnts_per_bucket: Vec<(u16, Color)> = (1..=num_colors)
        .map(|i| {
            cnt_distrib
                .percentile(100.0 * (i as f64) / (num_colors as f64))
                .unwrap()
        })
        .zip(colors.into_iter())
        .collect();

    // Now draw rectangles
    let square = Polygon::rectangle(opts.resolution, opts.resolution);
    for y in 0..counts.height {
        for x in 0..counts.width {
            let idx = counts.idx(x, y);
            let cnt = counts.data[idx];
            if cnt > 0 {
                let mut color = max_cnts_per_bucket[0].1;
                for (max, c) in &max_cnts_per_bucket {
                    if cnt >= *max {
                        color = *c;
                    } else {
                        break;
                    }
                }

                batch.push(
                    color,
                    square.translate((x as f64) * opts.resolution, (y as f64) * opts.resolution),
                );
            }
        }
    }
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

    fn neighbors_8(&self, x: usize, y: usize) -> Vec<usize> {
        let mut indices = Vec::new();
        let x1 = if x == 0 { 0 } else { x - 1 };
        let x2 = if x == self.width - 1 { x } else { x + 1 };
        let y1 = if y == 0 { 0 } else { y - 1 };
        let y2 = if y == self.height - 1 { y } else { y + 1 };
        for x in x1..=x2 {
            for y in y1..=y2 {
                indices.push(self.idx(x, y));
            }
        }
        indices
    }
}
