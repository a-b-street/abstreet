use crate::{Canvas, Color, UserInput};
use geom::Polygon;
use glium::implement_vertex;

// Something that's been sent to the GPU already.
pub struct Drawable {
    pub(crate) vertex_buffer: glium::VertexBuffer<Vertex>,
    pub(crate) index_buffer: glium::IndexBuffer<u32>,
}

#[derive(Copy, Clone)]
pub(crate) struct Vertex {
    position: [f32; 2],
    // TODO Maybe pass color as a uniform instead
    color: [f32; 4],
}

implement_vertex!(Vertex, position, color);

// TODO Don't expose this directly
pub struct Prerender<'a> {
    pub(crate) display: &'a glium::Display,
}

impl<'a> Prerender<'a> {
    pub fn upload_borrowed(&self, list: Vec<(Color, &Polygon)>) -> Drawable {
        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for (color, poly) in list {
            let idx_offset = vertices.len();
            let (pts, raw_indices) = poly.raw_for_rendering();
            for pt in pts {
                vertices.push(Vertex {
                    position: [pt.x() as f32, pt.y() as f32],
                    color: color.0,
                });
            }
            for idx in raw_indices {
                indices.push((idx_offset + *idx) as u32);
            }
        }

        let vertex_buffer = glium::VertexBuffer::new(self.display, &vertices).unwrap();
        let index_buffer = glium::IndexBuffer::new(
            self.display,
            glium::index::PrimitiveType::TrianglesList,
            &indices,
        )
        .unwrap();

        Drawable {
            vertex_buffer,
            index_buffer,
        }
    }

    pub fn upload(&self, list: Vec<(Color, Polygon)>) -> Drawable {
        let borrows = list.iter().map(|(c, p)| (*c, p)).collect();
        self.upload_borrowed(borrows)
    }
}

pub struct EventCtx<'a> {
    pub input: &'a mut UserInput,
    // TODO These two probably shouldn't be public
    pub canvas: &'a mut Canvas,
    pub prerender: &'a Prerender<'a>,
}
