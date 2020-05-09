use crate::drawing::Uniforms;
use crate::{Canvas, Color, FancyColor, ScreenDims, ScreenRectangle};
use geom::Polygon;
use glium::uniforms::UniformValue;
use glium::Surface;
use std::cell::Cell;

pub fn setup(
    window_title: &str,
) -> (
    PrerenderInnards,
    winit::event_loop::EventLoop<()>,
    ScreenDims,
) {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title(window_title)
        .with_maximized(true);
    // multisampling: 2 looks bad, 4 looks fine
    let context = glutin::ContextBuilder::new()
        .with_multisampling(4)
        .with_depth_buffer(2);
    let display = glium::Display::new(window, context, &event_loop).unwrap();

    let (vertex_shader, fragment_shader) =
        if display.is_glsl_version_supported(&glium::Version(glium::Api::Gl, 1, 4)) {
            (
                include_str!("shaders/vertex_140.glsl"),
                include_str!("shaders/fragment_140.glsl"),
            )
        } else {
            panic!(
                "GLSL 140 not supported. Try {:?} or {:?}",
                display.get_opengl_version(),
                display.get_supported_glsl_version()
            );
        };

    // To quickly iterate on shaders without recompiling...
    /*let mut vert = String::new();
    let mut frag = String::new();
    let (vertex_shader, fragment_shader) = {
        use std::io::Read;

        let mut f1 = std::fs::File:: open("../ezgui/src/shaders/vertex_140.glsl").unwrap();
        f1.read_to_string(&mut vert).unwrap();

        let mut f2 = std::fs::File:: open("../ezgui/src/shaders/fragment_140.glsl").unwrap();
        f2.read_to_string(&mut frag).unwrap();

        (&vert, &frag)
    };*/

    let program = glium::Program::new(
        &display,
        glium::program::ProgramCreationInput::SourceCode {
            vertex_shader,
            tessellation_control_shader: None,
            tessellation_evaluation_shader: None,
            geometry_shader: None,
            fragment_shader,
            transform_feedback_varyings: None,
            // Without this, SRGB gets enabled and post-processes the color from the fragment
            // shader.
            outputs_srgb: true,
            uses_point_size: false,
        },
    )
    .unwrap();

    let window_size = event_loop.primary_monitor().size();
    (
        PrerenderInnards {
            display,
            program,
            total_bytes_uploaded: Cell::new(0),
        },
        event_loop,
        ScreenDims::new(window_size.width.into(), window_size.height.into()),
    )
}

struct InnerUniforms<'a> {
    values: &'a Uniforms,
}

impl<'b> glium::uniforms::Uniforms for InnerUniforms<'b> {
    fn visit_values<'a, F: FnMut(&str, UniformValue<'a>)>(&'a self, mut output: F) {
        output("transform", UniformValue::Vec3(self.values.transform));
        output("window", UniformValue::Vec3(self.values.window));
    }
}

// Represents one frame that's gonna be drawn
pub struct GfxCtxInnards<'a> {
    target: glium::Frame,
    params: glium::DrawParameters<'a>,
}

impl<'a> GfxCtxInnards<'a> {
    pub fn clear(&mut self, c: Color) {
        // Without this, SRGB gets enabled and post-processes the color from the fragment
        // shader.
        self.target
            .clear_color_srgb_and_depth((c.r, c.g, c.b, c.a), 1.0);
    }

    pub fn redraw(&mut self, obj: &Drawable, uniforms: &Uniforms, prerender: &PrerenderInnards) {
        self.target
            .draw(
                &obj.vertex_buffer,
                &obj.index_buffer,
                &prerender.program,
                &InnerUniforms { values: uniforms },
                &self.params,
            )
            .unwrap();
    }

    pub fn enable_clipping(&mut self, rect: ScreenRectangle, canvas: &Canvas) {
        assert!(self.params.scissor.is_none());
        // The scissor rectangle has to be in device coordinates, so you would think some transform
        // by scale factor (previously called HiDPI factor) has to happen here. But actually,
        // window dimensions and the rectangle passed in are already scaled up. So don't do
        // anything here!
        self.params.scissor = Some(glium::Rect {
            left: rect.x1 as u32,
            // Y-inversion
            bottom: (canvas.window_height - rect.y2) as u32,
            width: (rect.x2 - rect.x1) as u32,
            height: (rect.y2 - rect.y1) as u32,
        });
    }

    pub fn disable_clipping(&mut self, _: &Canvas) {
        assert!(self.params.scissor.is_some());
        self.params.scissor = None;
    }

    pub fn take_clip(&mut self) -> Option<glium::Rect> {
        self.params.scissor.take()
    }
    pub fn restore_clip(&mut self, clip: Option<glium::Rect>) {
        self.params.scissor = clip;
    }

    pub fn finish(self) {
        self.target.finish().unwrap();
    }
}

// Something that's been sent to the GPU already.
pub struct Drawable {
    vertex_buffer: glium::VertexBuffer<Vertex>,
    index_buffer: glium::IndexBuffer<u32>,
}

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 2],
    // Each type of Color encodes something different here. See the actually_upload method and
    // fragment_140.glsl.
    // TODO Make this u8?
    style: [f32; 4],
}

glium::implement_vertex!(Vertex, position, style);

pub struct PrerenderInnards {
    display: glium::Display,
    program: glium::Program,

    // TODO Prerender doesn't know what things are temporary and permanent. Could make the API more
    // detailed.
    pub total_bytes_uploaded: Cell<usize>,
}

impl PrerenderInnards {
    pub fn actually_upload(&self, permanent: bool, list: Vec<(FancyColor, &Polygon)>) -> Drawable {
        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for (color, poly) in list {
            let idx_offset = vertices.len();
            let (pts, raw_indices) = poly.raw_for_rendering();
            for pt in pts {
                vertices.push(Vertex {
                    position: [pt.x() as f32, pt.y() as f32],
                    style: color.style(*pt),
                });
            }
            for idx in raw_indices {
                indices.push((idx_offset + *idx) as u32);
            }
        }

        let vertex_buffer = if permanent {
            glium::VertexBuffer::immutable(&self.display, &vertices).unwrap()
        } else {
            glium::VertexBuffer::new(&self.display, &vertices).unwrap()
        };
        let index_buffer = if permanent {
            glium::IndexBuffer::immutable(
                &self.display,
                glium::index::PrimitiveType::TrianglesList,
                &indices,
            )
            .unwrap()
        } else {
            glium::IndexBuffer::new(
                &self.display,
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

    pub fn request_redraw(&self) {
        self.display.gl_window().window().request_redraw();
    }

    pub fn set_cursor_icon(&self, icon: winit::window::CursorIcon) {
        self.display.gl_window().window().set_cursor_icon(icon);
    }

    pub fn draw_new_frame<'a>(&self) -> GfxCtxInnards<'a> {
        GfxCtxInnards {
            target: self.display.draw(),
            params: glium::DrawParameters {
                blend: glium::Blend::alpha_blending(),
                depth: glium::Depth {
                    test: glium::DepthTest::IfLessOrEqual,
                    write: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    }

    pub fn window_resized(&self, _: f64, _: f64) {}

    pub fn set_window_icon(&self, icon: winit::window::Icon) {
        self.display
            .gl_window()
            .window()
            .set_window_icon(Some(icon));
    }

    pub fn monitor_scale_factor(&self) -> f64 {
        self.display.gl_window().window().scale_factor()
    }
}
