use std::collections::{HashSet, VecDeque};

use geom::{Bounds, Distance, PolyLine, Polygon};
use map_gui::tools::Grid;
use map_model::Map;
use widgetry::{Color, GeomBatch};

use crate::{colors, Neighbourhood};

const RESOLUTION_M: f64 = 10.0;

pub struct RenderCells {
    /// Rarely, this might be empty if the area is very small
    pub polygons_per_cell: Vec<Vec<Polygon>>,
    /// Colors per cell, such that adjacent cells are colored differently
    pub colors: Vec<Color>,

    boundary_polygon: Polygon,
}

struct RenderCellsBuilder {
    /// The grid only covers the boundary polygon of the neighbourhood. The values are cell indices,
    /// and `Some(num_cells)` marks the boundary of the neighbourhood.
    grid: Grid<Option<usize>>,
    colors: Vec<Color>,
    /// Bounds of the neighbourhood boundary polygon
    bounds: Bounds,

    boundary_polygon: Polygon,
}

impl RenderCells {
    /// Partition a neighbourhood's boundary polygon based on the cells. This discretizes space into
    /// a grid, and then extracts a polygon from the raster. The results don't look perfect, but
    /// it's fast.
    pub fn new(map: &Map, neighbourhood: &Neighbourhood) -> RenderCells {
        RenderCellsBuilder::new(map, neighbourhood).finalize()
    }

    /// Draw cells as areas with different colors. The colors are meaningless, but the same color
    /// won't be shared between adjacent cells.
    pub fn draw_colored_areas(&self) -> GeomBatch {
        let mut batch = GeomBatch::new();
        for (color, polygons) in self.colors.iter().zip(self.polygons_per_cell.iter()) {
            for poly in polygons {
                batch.push(*color, poly.clone());
            }
        }
        batch
    }

    /// Draw the boundary between cells as a thick outline. It's meant to look like the
    /// neighbourhood is split into disconnected islands.
    pub fn draw_island_outlines(&self) -> GeomBatch {
        let neighbourhood_boundary = self
            .boundary_polygon
            .get_outer_ring()
            .map(|r| r.to_outline(Distance::meters(25.0)));

        let mut batch = GeomBatch::new();
        for (cell_color, polygons) in self.colors.iter().zip(self.polygons_per_cell.iter()) {
            for poly in polygons {
                // If the cell is disconnected, keep drawing this as an area to point out the
                // problem
                if *cell_color == colors::DISCONNECTED_CELL {
                    batch.push(*cell_color, poly.clone());
                    continue;
                }

                let boundary = PolyLine::unchecked_new(poly.clone().into_points())
                    .make_polygons(Distance::meters(5.0));

                let color = cell_color.alpha(1.0).shade(0.2);
                // If possible, try to erase where the cell boundary touches the perimeter road.
                if let Some(ref neighbourhood_boundary) = neighbourhood_boundary {
                    if let Ok(list) = boundary.difference(neighbourhood_boundary) {
                        batch.extend(color, list);
                    }
                    continue;
                }
                batch.push(color, boundary);
            }
        }
        batch
    }

    /// Per cell, convert all polygons to a `geo::MultiPolygon`. Leave the coordinate system as map-space.
    pub fn to_multipolygons(&self) -> Vec<geo::MultiPolygon> {
        self.polygons_per_cell
            .clone()
            .into_iter()
            .map(Polygon::union_all_into_multipolygon)
            .collect()
    }
}

impl RenderCellsBuilder {
    fn new(map: &Map, neighbourhood: &Neighbourhood) -> RenderCellsBuilder {
        let boundary_polygon = neighbourhood
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
        let mut warn_leak = true;
        for (cell_idx, cell) in neighbourhood.cells.iter().enumerate() {
            for (r, interval) in &cell.roads {
                let road = map.get_r(*r);
                // Some roads with a filter are _very_ short, and this fails. The connecting roads
                // on either side should contribute a grid cell and wind up fine.
                if let Ok(slice) = road
                    .center_pts
                    .maybe_exact_slice(interval.start, interval.end)
                {
                    // Walk along the center line. We could look at the road's thickness and fill
                    // out points based on that, but the diffusion should take care of it.
                    for (pt, _) in
                        slice.step_along(Distance::meters(RESOLUTION_M / 2.0), Distance::ZERO)
                    {
                        let grid_idx = grid.idx(
                            ((pt.x() - bounds.min_x) / RESOLUTION_M) as usize,
                            ((pt.y() - bounds.min_y) / RESOLUTION_M) as usize,
                        );
                        // Due to tunnels/bridges, sometimes a road belongs to a neighbourhood, but
                        // leaks outside the neighbourhood's boundary. Avoid crashing. The real fix
                        // is to better define boundaries in the face of z-order changes.
                        //
                        // Example is https://www.openstreetmap.org/way/87298633
                        if grid_idx >= grid.data.len() {
                            if warn_leak {
                                warn!(
                                    "{} leaks outside its neighbourhood's boundary polygon, near {}",
                                    road.id, pt
                                );
                                // In some neighbourhoods, there are so many warnings that logging
                                // causes noticeable slowdown!
                                warn_leak = false;
                            }
                            continue;
                        }

                        // If roads from two different cells are close enough to clobber
                        // originally, oh well?
                        grid.data[grid_idx] = Some(cell_idx);
                    }
                }
            }
        }
        // Also mark the boundary polygon, so we can prevent the diffusion from "leaking" outside
        // the area. The grid covers the rectangular bounds of the polygon. Rather than make an
        // enum with 3 cases, just assign a new index to mean "boundary."
        let boundary_marker = neighbourhood.cells.len();
        for (pt, _) in
            geom::PolyLine::unchecked_new(boundary_polygon.clone().into_ring().into_points())
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
        let mut cell_colors = color_cells(neighbourhood.cells.len(), adjacencies);

        // Color some special cells
        for (idx, cell) in neighbourhood.cells.iter().enumerate() {
            if cell.is_disconnected() {
                cell_colors[idx] = colors::DISCONNECTED_CELL;
            }
        }

        RenderCellsBuilder {
            grid,
            colors: cell_colors,
            bounds,

            boundary_polygon,
        }
    }

    fn finalize(self) -> RenderCells {
        let mut result = RenderCells {
            polygons_per_cell: Vec::new(),
            colors: Vec::new(),
            boundary_polygon: self.boundary_polygon,
        };

        for (idx, color) in self.colors.into_iter().enumerate() {
            // contour will find where the grid is >= a threshold value. The main grid has one
            // number per cell, so we can't directly use it -- the area >= some cell index is
            // meaningless. Per cell, make a new grid that just has that cell.
            let grid: Grid<f64> = Grid {
                width: self.grid.width,
                height: self.grid.height,
                data: self
                    .grid
                    .data
                    .iter()
                    .map(
                        |maybe_cell| {
                            if maybe_cell == &Some(idx) {
                                1.0
                            } else {
                                0.0
                            }
                        },
                    )
                    .collect(),
            };

            let smooth = false;
            let c = contour::ContourBuilder::new(grid.width as u32, grid.height as u32, smooth);
            let thresholds = vec![1.0];

            let mut cell_polygons = Vec::new();
            for feature in c.contours(&grid.data, &thresholds).unwrap() {
                match feature.geometry.unwrap().value {
                    geojson::Value::MultiPolygon(polygons) => {
                        for p in polygons {
                            if let Ok(poly) = Polygon::from_geojson(&p) {
                                cell_polygons.push(
                                    poly.must_scale(RESOLUTION_M)
                                        .translate(self.bounds.min_x, self.bounds.min_y),
                                );
                            }
                        }
                    }
                    _ => unreachable!(),
                }
            }

            // Sometimes one cell "leaks" out of the neighbourhood boundary. Not sure why. But we
            // can just clip the result.
            let mut clipped = Vec::new();
            for p in cell_polygons {
                // If clipping fails, just use the original polygon.
                if let Ok(list) = p.intersection(&result.boundary_polygon) {
                    clipped.extend(list);
                } else {
                    clipped.push(p);
                }
            }

            result.polygons_per_cell.push(clipped);
            result.colors.push(color);
        }

        result
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
        let mut available_colors: Vec<bool> =
            std::iter::repeat(true).take(colors::CELLS.len()).collect();
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

        // If there are multiple colors available, prefer one that hasn't been used anywhere yet.
        // Cells far apart shouldn't seem related to the user.
        let mut choice = None;
        let mut backup = None;
        for (idx, available) in available_colors.into_iter().enumerate() {
            if !available {
                continue;
            }
            if assigned_colors.iter().any(|x| *x == idx) {
                if backup.is_none() {
                    backup = Some(idx);
                }
            } else {
                choice = Some(idx);
                break;
            }
        }
        assigned_colors.push(
            choice
                .or(backup)
                .unwrap_or_else(|| assigned_colors.len() % colors::CELLS.len()),
        );
    }
    assigned_colors
        .into_iter()
        .map(|idx| colors::CELLS[idx].alpha(0.8))
        .collect()
}
