use std::collections::VecDeque;

use geom::{Distance, Polygon, Pt2D};
use map_gui::tools::Grid;
use map_model::Map;
use widgetry::{Color, GeomBatch};

use super::Neighborhood;

pub const COLORS: [Color; 6] = [
    Color::BLUE,
    Color::YELLOW,
    Color::GREEN,
    Color::PURPLE,
    Color::PINK,
    Color::ORANGE,
];

/// Partition a neighborhood's boundary polygon based on the cells. Currently this discretizes
/// space into a grid, so the results don't look perfect, but it's fast.
pub fn draw_cells(map: &Map, neighborhood: &Neighborhood) -> GeomBatch {
    let boundary_polygon = neighborhood
        .orig_perimeter
        .clone()
        .to_block(map)
        .unwrap()
        .polygon;
    // Make a 2D grid covering the polygon. Each tile in the grid contains a cell index, which will
    // become a color by the end. None means no cell is assigned yet.
    let bounds = boundary_polygon.get_bounds();
    let resolution_m = 10.0;
    let mut grid: Grid<Option<usize>> = Grid::new(
        (bounds.width() / resolution_m).ceil() as usize,
        (bounds.height() / resolution_m).ceil() as usize,
        None,
    );

    // Initially fill out the grid based on the roads in each cell
    for (cell_idx, cell) in neighborhood.cells.iter().enumerate() {
        for (r, interval) in &cell.roads {
            let road = map.get_r(*r);
            // Walk along the center line. We could look at the road's thickness and fill out
            // points based on that, but the diffusion should take care of it.
            for (pt, _) in road
                .center_pts
                .exact_slice(interval.start, interval.end)
                .step_along(Distance::meters(resolution_m / 2.0), Distance::ZERO)
            {
                let grid_idx = grid.idx(
                    ((pt.x() - bounds.min_x) / resolution_m) as usize,
                    ((pt.y() - bounds.min_y) / resolution_m) as usize,
                );
                // If roads from two different cells are close enough to clobber originally, oh
                // well?
                grid.data[grid_idx] = Some(cell_idx);
            }
        }
    }
    // Also mark the boundary polygon, so we can prevent the diffusion from "leaking" outside the
    // area. The grid covers the rectangular bounds of the polygon. Rather than make an enum with 3
    // cases, just assign a new index to mean "boundary."
    let boundary_marker = neighborhood.cells.len();
    for (pt, _) in geom::PolyLine::unchecked_new(boundary_polygon.into_ring().into_points())
        .step_along(Distance::meters(resolution_m / 2.0), Distance::ZERO)
    {
        // TODO Refactor helpers to transform between map-space and the grid tiles. Possibly Grid
        // should know about this.
        let grid_idx = grid.idx(
            ((pt.x() - bounds.min_x) / resolution_m) as usize,
            ((pt.y() - bounds.min_y) / resolution_m) as usize,
        );
        grid.data[grid_idx] = Some(boundary_marker);
    }

    diffusion(&mut grid, boundary_marker);

    // Just draw rectangles based on the grid
    // TODO We should be able to generate actual polygons per cell using the contours crate
    // TODO Also it'd look nicer to render this "underneath" the roads and intersections, at the
    // layer where areas are shown now
    let mut batch = GeomBatch::new();
    for (idx, value) in grid.data.iter().enumerate() {
        if let Some(cell_idx) = value {
            if *cell_idx == boundary_marker {
                continue;
            }
            let (x, y) = grid.xy(idx);
            let tile_center = Pt2D::new(
                bounds.min_x + resolution_m * (x as f64 + 0.5),
                bounds.min_y + resolution_m * (y as f64 + 0.5),
            );
            // TODO If two cells are adjacent, assign different colors
            let color = COLORS[cell_idx % COLORS.len()].alpha(0.5);
            batch.push(
                color,
                Polygon::rectangle_centered(
                    tile_center,
                    Distance::meters(resolution_m),
                    Distance::meters(resolution_m),
                ),
            );
        }
    }
    batch
}

fn diffusion(grid: &mut Grid<Option<usize>>, boundary_marker: usize) {
    // Grid indices to propagate
    let mut queue: VecDeque<usize> = VecDeque::new();

    // Initially seed the queue with all colored tiles
    for (idx, value) in grid.data.iter().enumerate() {
        if let Some(x) = value {
            // Don't expand the boundary tiles
            if *x != boundary_marker {
                queue.push_back(idx);
            }
        }
    }

    while !queue.is_empty() {
        let current_idx = queue.pop_front().unwrap();
        let current_color = grid.data[current_idx].unwrap();
        let (current_x, current_y) = grid.xy(current_idx);
        // Don't flood to diagonal neighbors. That would usually result in "leaking" out past the
        // boundary tiles when the boundary polygon isn't axis-aligned.
        // TODO But this still does "leak" out sometimes -- the cell covering 22nd/Lynn, for
        // example.
        for (next_x, next_y) in grid.orthogonal_neighbors(current_x, current_y) {
            let next_idx = grid.idx(next_x, next_y);
            if grid.data[next_idx].is_none() {
                grid.data[next_idx] = Some(current_color);
                queue.push_back(next_idx);
            }
            // If a color has been assigned, don't flood any further. If the color doesn't match
            // our current_color, we've found the border between two cells. But we don't need to do
            // anything with that border.
        }
    }
}
