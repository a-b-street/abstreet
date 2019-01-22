use genmesh;
use glium::vertex::VertexBufferAny;
use glium::{self, implement_vertex, Display};
use obj;

/// Returns a vertex buffer that should be rendered as `TrianglesList`.
pub fn load_wavefront(display: &Display, data: &[u8]) -> VertexBufferAny {
    #[derive(Copy, Clone)]
    struct Vertex {
        position: [f32; 3],
        normal: [f32; 3],
        texture: [f32; 2],
    }

    implement_vertex!(Vertex, position, normal, texture);

    let mut data = ::std::io::BufReader::new(data);
    let data = obj::Obj::load_buf(&mut data).unwrap();

    let mut vertex_data = Vec::new();

    for object in data.objects.iter() {
        for polygon in object.groups.iter().flat_map(|g| g.polys.iter()) {
            match polygon {
                &genmesh::Polygon::PolyTri(genmesh::Triangle {
                    x: v1,
                    y: v2,
                    z: v3,
                }) => {
                    for v in [v1, v2, v3].iter() {
                        let position = data.position[v.0];
                        let texture = v.1.map(|index| data.texture[index]);
                        let normal = v.2.map(|index| data.normal[index]);

                        let texture = texture.unwrap_or([0.0, 0.0]);
                        let normal = normal.unwrap_or([0.0, 0.0, 0.0]);

                        vertex_data.push(Vertex {
                            position: position,
                            normal: normal,
                            texture: texture,
                        })
                    }
                }
                _ => unimplemented!(),
            }
        }
    }

    glium::vertex::VertexBuffer::new(display, &vertex_data)
        .unwrap()
        .into_vertex_buffer_any()
}
