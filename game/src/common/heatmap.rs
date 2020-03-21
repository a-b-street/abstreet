use ezgui::{Color, GeomBatch};
use geom::{Bounds, Polygon, Pt2D};

pub fn make_heatmap(batch: &mut GeomBatch, bounds: &Bounds, pts: Vec<Pt2D>) {
    // Meters
    let resolution = 10.0;
    // u8 is not quite enough -- one building could totally have more than 256 people.
    let mut counts: Grid<u16> = Grid::new(
        (bounds.width() / resolution).ceil() as usize,
        (bounds.height() / resolution).ceil() as usize,
        0,
    );

    for pt in pts {
        // TODO more careful rounding
        let idx = counts.idx(
            ((pt.x() - bounds.min_x) / resolution) as usize,
            ((pt.y() - bounds.min_y) / resolution) as usize,
        );
        counts.data[idx] += 1;
    }

    // Diffusion
    let num_passes = 5;
    for _ in 0..num_passes {
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

    // Now draw rectangles
    let max = *counts.data.iter().max().unwrap();
    // TODO Full spectral progression isn't recommended anymore!
    // This is in order from low density to high.
    let colors = vec![
        Color::hex("#0b2c7a"),
        Color::hex("#1e9094"),
        Color::hex("#0ec441"),
        Color::hex("#7bed00"),
        Color::hex("#f7d707"),
        Color::hex("#e68e1c"),
        Color::hex("#c2523c"),
    ];
    // TODO Off by 1?
    let range = max / ((colors.len() - 1) as u16);
    if range == 0 {
        // Max is too low, use less colors?
        return;
    }
    let square = Polygon::rectangle(resolution, resolution);
    for y in 0..counts.height {
        for x in 0..counts.width {
            let idx = counts.idx(x, y);
            let cnt = counts.data[idx];
            if cnt > 0 {
                // TODO Urgh, uneven buckets
                let color = colors[((cnt / range) as usize).min(colors.len() - 1)];
                batch.push(
                    color,
                    square.translate((x as f64) * resolution, (y as f64) * resolution),
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
