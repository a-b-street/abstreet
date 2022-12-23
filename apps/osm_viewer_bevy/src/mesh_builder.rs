use bevy::prelude::Mesh;
use bevy_earcutr::{build_mesh_from_earcutr, EarcutrResult};
use geom::{Polygon, Tessellation};

pub fn build_mesh_from_polygon(polygon: Polygon) -> Mesh {
    let earcutr_output = Tessellation::from(polygon).consume();

    build_mesh_from_earcutr(
        EarcutrResult {
            vertices: earcutr_output
                .0
                .iter()
                .flat_map(|p| vec![p.x(), p.y()])
                .collect::<Vec<f64>>(),
            triangle_indices: earcutr_output
                .1
                .iter()
                .rev()
                .map(|i| *i as usize)
                .collect::<Vec<usize>>(),
        },
        0.,
    )
}
