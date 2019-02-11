use crate::{Canvas, Color, UserInput};
use geom::Polygon;
use glium::implement_vertex;
use std::cell::Cell;

// Something that's been sent to the GPU already.
pub struct Drawable {
    pub(crate) vertex_buffer: glium::VertexBuffer<Vertex>,
    pub(crate) index_buffer: glium::IndexBuffer<u32>,
}

#[derive(Copy, Clone)]
pub(crate) struct Vertex {
    position: [f32; 2],
    // TODO Maybe pass color as a uniform instead
    // TODO Or have a fixed palette of colors and just index into it
    color: [u8; 4],
}

implement_vertex!(Vertex, position, color);

// TODO Don't expose this directly
pub struct Prerender<'a> {
    pub(crate) display: &'a glium::Display,
    pub(crate) num_uploads: Cell<usize>,
    // TODO Prerender doesn't know what things are temporary and permanent. Could make the API more
    // detailed (and use the corresponding persistent glium types).
    pub(crate) total_bytes_uploaded: Cell<usize>,
}

impl<'a> Prerender<'a> {
    pub fn upload_borrowed(&self, list: Vec<(Color, &Polygon)>) -> Drawable {
        self.actually_upload(true, list)
    }

    pub fn upload(&self, list: Vec<(Color, Polygon)>) -> Drawable {
        let borrows = list.iter().map(|(c, p)| (*c, p)).collect();
        self.actually_upload(true, borrows)
    }

    pub fn get_total_bytes_uploaded(&self) -> usize {
        self.total_bytes_uploaded.get()
    }

    pub(crate) fn upload_temporary(&self, list: Vec<(Color, &Polygon)>) -> Drawable {
        self.actually_upload(false, list)
    }

    fn actually_upload(&self, permanent: bool, list: Vec<(Color, &Polygon)>) -> Drawable {
        self.num_uploads.set(self.num_uploads.get() + 1);

        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for (color, poly) in list {
            let idx_offset = vertices.len();
            let (pts, raw_indices) = poly.raw_for_rendering();
            for pt in pts {
                vertices.push(Vertex {
                    position: [pt.x() as f32, pt.y() as f32],
                    color: [
                        f32_to_u8(color.0[0]),
                        f32_to_u8(color.0[1]),
                        f32_to_u8(color.0[2]),
                        f32_to_u8(color.0[3]),
                    ],
                });
            }
            for idx in raw_indices {
                indices.push((idx_offset + *idx) as u32);
            }
        }

        let vertex_buffer = if permanent {
            glium::VertexBuffer::immutable(self.display, &vertices).unwrap()
        } else {
            glium::VertexBuffer::new(self.display, &vertices).unwrap()
        };
        let index_buffer = if permanent {
            glium::IndexBuffer::immutable(
                self.display,
                glium::index::PrimitiveType::TrianglesList,
                &indices,
            )
            .unwrap()
        } else {
            glium::IndexBuffer::new(
                self.display,
                glium::index::PrimitiveType::TrianglesList,
                &indices,
            )
            .unwrap()
        };

        if permanent {
            self.total_bytes_uploaded.set(
                self.total_bytes_uploaded.get()
                    + vertex_buffer.get_size()
                    + index_buffer.get_size(),
            );
        }

        Drawable {
            vertex_buffer,
            index_buffer,
        }
    }
}

pub struct EventCtx<'a> {
    pub input: &'a mut UserInput,
    // TODO These two probably shouldn't be public
    pub canvas: &'a mut Canvas,
    pub prerender: &'a Prerender<'a>,
}

fn f32_to_u8(x: f32) -> u8 {
    (x * 255.0) as u8
}
