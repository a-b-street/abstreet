use crate::{Canvas, Color, ScreenPt};
use dimensioned::si;
use geom::{Circle, Line, Polygon, Pt2D};
use glium::{implement_vertex, uniform, Surface};

const TRIANGLES_PER_CIRCLE: usize = 60;

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 2],
    // TODO Maybe pass color as a uniform instead
    color: [f32; 4],
}

implement_vertex!(Vertex, position, color);

type Uniforms<'a> = glium::uniforms::UniformsStorage<
    'a,
    [f32; 2],
    glium::uniforms::UniformsStorage<'a, [f32; 3], glium::uniforms::EmptyUniforms>,
>;

pub struct GfxCtx<'a> {
    display: &'a glium::Display,
    target: &'a mut glium::Frame,
    program: &'a glium::Program,
    uniforms: Uniforms<'a>,
    params: glium::DrawParameters<'a>,

    pub num_new_uploads: usize,
    pub num_draw_calls: usize,
}

impl<'a> GfxCtx<'a> {
    pub fn new(
        canvas: &Canvas,
        display: &'a glium::Display,
        target: &'a mut glium::Frame,
        program: &'a glium::Program,
    ) -> GfxCtx<'a> {
        let params = glium::DrawParameters {
            blend: glium::Blend::alpha_blending(),
            ..Default::default()
        };

        let uniforms = uniform! {
            transform: [canvas.cam_x as f32, canvas.cam_y as f32, canvas.cam_zoom as f32],
            window: [canvas.window_width as f32, canvas.window_height as f32],
        };

        GfxCtx {
            display,
            target,
            program,
            uniforms,
            params,
            num_new_uploads: 0,
            num_draw_calls: 0,
        }
    }

    // Up to the caller to call unfork()!
    // TODO Canvas doesn't understand this change, so things like text drawing that use
    // map_to_screen will just be confusing.
    pub fn fork(
        &mut self,
        top_left_map: Pt2D,
        top_left_screen: ScreenPt,
        zoom: f64,
        canvas: &Canvas,
    ) {
        // map_to_screen of top_left_map should be top_left_screen
        let cam_x = (top_left_map.x() * zoom) - top_left_screen.x;
        let cam_y = (top_left_map.y() * zoom) - top_left_screen.y;

        self.uniforms = uniform! {
            transform: [cam_x as f32, cam_y as f32, zoom as f32],
            window: [canvas.window_width as f32, canvas.window_height as f32],
        };
    }

    pub fn fork_screenspace(&mut self, canvas: &Canvas) {
        self.uniforms = uniform! {
            transform: [0.0, 0.0, 1.0],
            window: [canvas.window_width as f32, canvas.window_height as f32],
        };
    }

    pub fn unfork(&mut self, canvas: &Canvas) {
        self.uniforms = uniform! {
            transform: [canvas.cam_x as f32, canvas.cam_y as f32, canvas.cam_zoom as f32],
            window: [canvas.window_width as f32, canvas.window_height as f32],
        };
    }

    pub fn clear(&mut self, color: Color) {
        // Without this, SRGB gets enabled and post-processes the color from the fragment shader.
        self.target
            .clear_color_srgb(color.0[0], color.0[1], color.0[2], color.0[3]);
    }

    // Use graphics::Line internally for now, but make it easy to switch to something else by
    // picking this API now.
    pub fn draw_line(&mut self, color: Color, thickness: f64, line: &Line) {
        self.draw_polygon(color, &line.make_polygons(thickness));
    }

    pub fn draw_rounded_line(&mut self, color: Color, thickness: f64, line: &Line) {
        self.draw_polygon_batch(vec![
            (color, &line.make_polygons(thickness)),
            (
                color,
                &Circle::new(line.pt1(), thickness / 2.0).to_polygon(TRIANGLES_PER_CIRCLE),
            ),
            (
                color,
                &Circle::new(line.pt2(), thickness / 2.0).to_polygon(TRIANGLES_PER_CIRCLE),
            ),
        ]);
    }

    pub fn draw_arrow(&mut self, color: Color, thickness: f64, line: &Line) {
        let head_size = 2.0 * thickness;
        let angle = line.angle();
        let triangle_height = (head_size / 2.0).sqrt() * si::M;
        self.draw_polygon_batch(vec![
            (
                color,
                &Polygon::new(&vec![
                    //line.pt2(),
                    //line.pt2().project_away(head_size, angle.rotate_degs(-135.0)),
                    line.reverse()
                        .dist_along(triangle_height)
                        .project_away(thickness / 2.0, angle.rotate_degs(90.0)),
                    line.pt1()
                        .project_away(thickness / 2.0, angle.rotate_degs(90.0)),
                    line.pt1()
                        .project_away(thickness / 2.0, angle.rotate_degs(-90.0)),
                    line.reverse()
                        .dist_along(triangle_height)
                        .project_away(thickness / 2.0, angle.rotate_degs(-90.0)),
                    //line.pt2().project_away(head_size, angle.rotate_degs(135.0)),
                ]),
            ),
            (
                color,
                &Polygon::new(&vec![
                    line.pt2(),
                    line.pt2()
                        .project_away(head_size, angle.rotate_degs(-135.0)),
                    line.pt2().project_away(head_size, angle.rotate_degs(135.0)),
                ]),
            ),
        ]);
    }

    pub fn draw_circle(&mut self, color: Color, circle: &Circle) {
        self.draw_polygon(color, &circle.to_polygon(TRIANGLES_PER_CIRCLE));
    }

    pub fn draw_polygon(&mut self, color: Color, poly: &Polygon) {
        self.draw_polygon_batch(vec![(color, poly)]);
    }

    pub fn draw_polygon_batch(&mut self, list: Vec<(Color, &Polygon)>) {
        let obj = Prerender {
            display: self.display,
        }
        .upload(list);
        self.num_new_uploads += 1;
        self.redraw(&obj);
    }

    pub fn redraw(&mut self, obj: &Drawable) {
        self.target
            .draw(
                &obj.vertex_buffer,
                &obj.index_buffer,
                &self.program,
                &self.uniforms,
                &self.params,
            )
            .unwrap();
        self.num_draw_calls += 1;
    }
}

pub struct Prerender<'a> {
    pub(crate) display: &'a glium::Display,
}

// Something that's been sent to the GPU already.
pub struct Drawable {
    vertex_buffer: glium::VertexBuffer<Vertex>,
    index_buffer: glium::IndexBuffer<u32>,
}

impl<'a> Prerender<'a> {
    // TODO Taking &Polygon is annoying for the callers
    // TODO One color, many polygons could also be helpful
    pub fn upload(&self, list: Vec<(Color, &Polygon)>) -> Drawable {
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
}
