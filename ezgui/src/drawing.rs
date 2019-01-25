use crate::{Canvas, Color, ScreenPt};
use dimensioned::si;
use geom::{Circle, Line, Polygon, Pt2D};
use glium::{implement_vertex, uniform, Surface};

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
        self.draw_polygon(color, &line.to_polyline().make_polygons(thickness));
    }

    pub fn draw_rounded_line(&mut self, color: Color, thickness: f64, line: &Line) {
        self.draw_line(color, thickness, line);
        self.draw_circle(color, &Circle::new(line.pt1(), thickness / 2.0));
        self.draw_circle(color, &Circle::new(line.pt2(), thickness / 2.0));
    }

    pub fn draw_arrow(&mut self, color: Color, thickness: f64, line: &Line) {
        let head_size = 2.0 * thickness;
        let angle = line.angle();
        let triangle_height = (head_size / 2.0).sqrt() * si::M;
        self.draw_polygon(
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
        );
        self.draw_polygon(
            color,
            &Polygon::new(&vec![
                line.pt2(),
                line.pt2()
                    .project_away(head_size, angle.rotate_degs(-135.0)),
                line.pt2().project_away(head_size, angle.rotate_degs(135.0)),
            ]),
        );
    }

    pub fn draw_polygon(&mut self, color: Color, poly: &Polygon) {
        let (pts, raw_indices) = poly.raw_for_rendering();
        let vertices: Vec<Vertex> = pts
            .iter()
            .map(|pt| Vertex {
                position: [pt.x() as f32, pt.y() as f32],
                color: color.0,
            })
            .collect();
        let indices: Vec<u32> = raw_indices.iter().map(|i| *i as u32).collect();

        let vertex_buffer = glium::VertexBuffer::new(self.display, &vertices).unwrap();
        let index_buffer = glium::IndexBuffer::new(
            self.display,
            glium::index::PrimitiveType::TrianglesList,
            &indices,
        )
        .unwrap();

        self.target
            .draw(
                &vertex_buffer,
                &index_buffer,
                &self.program,
                &self.uniforms,
                &self.params,
            )
            .unwrap();
    }

    pub fn draw_circle(&mut self, color: Color, circle: &Circle) {
        self.draw_polygon(color, &circle.to_polygon(60));
    }
}
