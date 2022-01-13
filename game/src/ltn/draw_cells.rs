use std::collections::{HashSet, VecDeque};

use abstutil::Timer;
use geom::{Bounds, Distance, Polygon, Pt2D};
use map_gui::tools::Grid;
use map_model::Map;
use widgetry::{Color, GeomBatch};

use super::Neighborhood;

lazy_static::lazy_static! {
    static ref COLORS: [Color; 6] = [
        Color::BLUE,
        Color::YELLOW,
        Color::hex("#3CAEA3"),
        Color::PURPLE,
        Color::PINK,
        Color::ORANGE,
    ];
}
const CAR_FREE_COLOR: Color = Color::GREEN;
const DISCONNECTED_COLOR: Color = Color::RED;

const RESOLUTION_M: f64 = 10.0;

pub struct RenderCells {
    /// The grid only covers the boundary polygon of the neighborhood. The values are cell indices,
    /// and `Some(num_cells)` marks the boundary of the neighborhood.
    grid: Grid<Option<usize>>,
    /// Colors per cell, such that adjacent cells are colored differently
    pub colors: Vec<Color>,
    /// Bounds of the neighborhood boundary polygon
    bounds: Bounds,
    /// The number of cells, used as a sentinel value in the grid
    boundary_marker: usize,
}

/// Partition a neighborhood's boundary polygon based on the cells. This discretizes
/// space into a grid, so the results don't look perfect, but it's fast.
impl RenderCells {
    pub fn new(map: &Map, neighborhood: &Neighborhood) -> RenderCells {
        let boundary_polygon = neighborhood
            .orig_perimeter
            .clone()
            .to_block(map)
            .unwrap()
            .polygon;
        // Make a 2D grid covering the polygon. Each tile in the grid contains a cell index, which
        // will become a color by the end. None means no cell is assigned yet.
        let bounds = boundary_polygon.get_bounds();
        let mut grid: Grid<Option<usize>> = Grid::new(
            (bounds.width() / RESOLUTION_M).ceil() as usize,
            (bounds.height() / RESOLUTION_M).ceil() as usize,
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
                    .step_along(Distance::meters(RESOLUTION_M / 2.0), Distance::ZERO)
                {
                    let grid_idx = grid.idx(
                        ((pt.x() - bounds.min_x) / RESOLUTION_M) as usize,
                        ((pt.y() - bounds.min_y) / RESOLUTION_M) as usize,
                    );
                    // If roads from two different cells are close enough to clobber originally, oh
                    // well?
                    grid.data[grid_idx] = Some(cell_idx);
                }
            }
        }
        // Also mark the boundary polygon, so we can prevent the diffusion from "leaking" outside
        // the area. The grid covers the rectangular bounds of the polygon. Rather than make an
        // enum with 3 cases, just assign a new index to mean "boundary."
        let boundary_marker = neighborhood.cells.len();
        for (pt, _) in geom::PolyLine::unchecked_new(boundary_polygon.into_ring().into_points())
            .step_along(Distance::meters(RESOLUTION_M / 2.0), Distance::ZERO)
        {
            // TODO Refactor helpers to transform between map-space and the grid tiles. Possibly
            // Grid should know about this.
            let grid_idx = grid.idx(
                ((pt.x() - bounds.min_x) / RESOLUTION_M) as usize,
                ((pt.y() - bounds.min_y) / RESOLUTION_M) as usize,
            );
            grid.data[grid_idx] = Some(boundary_marker);
        }

        let adjacencies = diffusion(&mut grid, boundary_marker);
        let mut cell_colors = color_cells(neighborhood.cells.len(), adjacencies);

        // Color car-free cells in a special way
        for (idx, cell) in neighborhood.cells.iter().enumerate() {
            if cell.car_free {
                cell_colors[idx] = CAR_FREE_COLOR;
            } else if cell.is_disconnected() {
                cell_colors[idx] = DISCONNECTED_COLOR;
            }
        }

        RenderCells {
            grid,
            colors: cell_colors,
            bounds,
            boundary_marker,
        }
    }

    /// Just draw rectangles based on the grid
    pub fn draw_grid(&self) -> GeomBatch {
        // TODO We should be able to generate actual polygons per cell using the contours crate
        // TODO Also it'd look nicer to render this "underneath" the roads and intersections, at the
        // layer where areas are shown now
        let mut batch = GeomBatch::new();
        for (idx, value) in self.grid.data.iter().enumerate() {
            if let Some(cell_idx) = value {
                if *cell_idx == self.boundary_marker {
                    continue;
                }
                let (x, y) = self.grid.xy(idx);
                let tile_center = Pt2D::new(
                    self.bounds.min_x + RESOLUTION_M * (x as f64 + 0.5),
                    self.bounds.min_y + RESOLUTION_M * (y as f64 + 0.5),
                );
                batch.push(
                    self.colors[*cell_idx].alpha(0.5),
                    Polygon::rectangle_centered(
                        tile_center,
                        Distance::meters(RESOLUTION_M),
                        Distance::meters(RESOLUTION_M),
                    ),
                );
            }
        }
        batch
    }

    /// Per cell, glue together all of the rectangles into a single multipolygon
    pub fn to_multipolygons(&self, timer: &mut Timer) -> Vec<geo::MultiPolygon<f64>> {
        let mut polygons_per_cell: Vec<Vec<Polygon>> = std::iter::repeat_with(Vec::new)
            .take(self.boundary_marker)
            .collect();
        for (idx, value) in self.grid.data.iter().enumerate() {
            if let Some(cell_idx) = value {
                if *cell_idx == self.boundary_marker {
                    continue;
                }
                let (x, y) = self.grid.xy(idx);
                let tile_center = Pt2D::new(
                    self.bounds.min_x + RESOLUTION_M * (x as f64 + 0.5),
                    self.bounds.min_y + RESOLUTION_M * (y as f64 + 0.5),
                );
                polygons_per_cell[*cell_idx].push(Polygon::rectangle_centered(
                    tile_center,
                    Distance::meters(RESOLUTION_M),
                    Distance::meters(RESOLUTION_M),
                ));
            }
        }

        timer.parallelize(
            "Unioning polygons for one cell",
            polygons_per_cell,
            Polygon::union_all_into_multipolygon,
        )
    }
}

/// Returns a set of adjacent indices. The pairs are symmetric -- (x, y) and (y, x) will both be
/// populated. Adjacency with boundary_marker doesn't count.
fn diffusion(grid: &mut Grid<Option<usize>>, boundary_marker: usize) -> HashSet<(usize, usize)> {
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

    let mut adjacencies = HashSet::new();

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
            if let Some(prev_color) = grid.data[next_idx] {
                // If the color doesn't match our current_color, we've found the border between two
                // cells.
                if current_color != prev_color
                    && current_color != boundary_marker
                    && prev_color != boundary_marker
                {
                    adjacencies.insert((current_color, prev_color));
                    adjacencies.insert((prev_color, current_color));
                }
                // If a color has been assigned, don't flood any further.
            } else {
                grid.data[next_idx] = Some(current_color);
                queue.push_back(next_idx);
            }
        }
    }

    adjacencies
}

fn color_cells(num_cells: usize, adjacencies: HashSet<(usize, usize)>) -> Vec<Color> {
    // This is the same greedy logic as Perimeter::calculate_coloring
    let mut assigned_colors = Vec::new();
    for this_idx in 0..num_cells {
        let mut available_colors: Vec<bool> = std::iter::repeat(true).take(COLORS.len()).collect();
        // Find all neighbors
        for other_idx in 0..num_cells {
            if adjacencies.contains(&(this_idx, other_idx)) {
                // We assign colors in order, so any neighbor index smaller than us has been
                // chosen
                if other_idx < this_idx {
                    available_colors[assigned_colors[other_idx]] = false;
                }
            }
        }
        if let Some(color) = available_colors.iter().position(|x| *x) {
            assigned_colors.push(color);
        } else {
            warn!("color_cells ran out of colors");
            assigned_colors.push(0);
        }
    }
    assigned_colors.into_iter().map(|idx| COLORS[idx]).collect()
}
