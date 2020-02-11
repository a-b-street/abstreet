use crate::drawing::Uniforms;
use crate::{Canvas, Color, ScreenDims, ScreenRectangle, TextureType};
use geom::{Angle, Polygon, Pt2D};
use glium::texture::{RawImage2d, Texture2dArray};
use glium::uniforms::{SamplerBehavior, SamplerWrapFunction, UniformValue};
use glium::Surface;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};

pub fn setup(window_title: &str) -> (PrerenderInnards, winit::event_loop::EventLoop<()>) {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title(window_title)
        .with_maximized(true);
    // multisampling: 2 looks bad, 4 looks fine
    let context = glutin::ContextBuilder::new()
        .with_multisampling(4)
        .with_depth_buffer(2);
    // TODO This step got slow
    println!("Initializing OpenGL window");
    let display = glium::Display::new(window, context, &event_loop).unwrap();

    let (vertex_shader, fragment_shader) =
        if display.is_glsl_version_supported(&glium::Version(glium::Api::Gl, 1, 4)) {
            (
                include_str!("assets/vertex_140.glsl"),
                include_str!("assets/fragment_140.glsl"),
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

        let mut f1 = std::fs::File:: open("../ezgui/src/assets/vertex_140.glsl").unwrap();
        f1.read_to_string(&mut vert).unwrap();

        let mut f2 = std::fs::File:: open("../ezgui/src/assets/fragment_140.glsl").unwrap();
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

    (
        PrerenderInnards {
            display,
            program,
            total_bytes_uploaded: Cell::new(0),
            texture_arrays: RefCell::new(Vec::new()),
            texture_lookups: RefCell::new(HashMap::new()),
        },
        event_loop,
    )
}

struct InnerUniforms<'a> {
    values: &'a Uniforms,
    arrays: &'a Vec<Texture2dArray>,
}

impl<'b> glium::uniforms::Uniforms for InnerUniforms<'b> {
    fn visit_values<'a, F: FnMut(&str, UniformValue<'a>)>(&'a self, mut output: F) {
        output("transform", UniformValue::Vec3(self.values.transform));
        output("window", UniformValue::Vec3(self.values.window));

        // This is fine to use for all of the texture styles; all but non-tiling textures clamp to
        // [0, 1] anyway.
        let tile = SamplerBehavior {
            wrap_function: (
                SamplerWrapFunction::Repeat,
                SamplerWrapFunction::Repeat,
                SamplerWrapFunction::Repeat,
            ),
            ..Default::default()
        };
        for (idx, tex) in self.arrays.iter().enumerate() {
            output(
                &format!("tex{}", idx),
                UniformValue::Texture2dArray(tex, Some(tile)),
            );
        }
    }
}

// Represents one frame that's gonna be drawn
pub struct GfxCtxInnards<'a> {
    target: glium::Frame,
    params: glium::DrawParameters<'a>,
}

impl<'a> GfxCtxInnards<'a> {
    pub fn clear(&mut self, color: Color) {
        match color {
            Color::RGBA(r, g, b, a) => {
                // Without this, SRGB gets enabled and post-processes the color from the fragment
                // shader.
                self.target.clear_color_srgb_and_depth((r, g, b, a), 1.0);
            }
            _ => unreachable!(),
        }
    }

    pub fn redraw(&mut self, obj: &Drawable, uniforms: &Uniforms, prerender: &PrerenderInnards) {
        self.target
            .draw(
                &obj.vertex_buffer,
                &obj.index_buffer,
                &prerender.program,
                &InnerUniforms {
                    values: uniforms,
                    arrays: &prerender.texture_arrays.borrow(),
                },
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

    pub fn disable_clipping(&mut self) {
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
    // detailed (and use the corresponding persistent glium types).
    pub total_bytes_uploaded: Cell<usize>,

    // Kind of a weird place for this, but ah well.
    texture_arrays: RefCell<Vec<Texture2dArray>>,
    pub texture_lookups: RefCell<HashMap<String, Color>>,
}

impl PrerenderInnards {
    pub fn actually_upload(&self, permanent: bool, list: Vec<(Color, &Polygon)>) -> Drawable {
        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for (color, poly) in list {
            let idx_offset = vertices.len();
            let (pts, raw_indices) = poly.raw_for_rendering();
            for pt in pts {
                // For the three texture cases, pass [U coordinate, V coordinate, texture group ID,
                // 100 + texture offset ID] as the style. The last field is between 0 an 1 RGBA's
                // alpha values, so bump by 100 to distinguish from that.
                let style = match color {
                    Color::RGBA(r, g, b, a) => [r, g, b, a],
                    Color::TileTexture(id, tex_dims) => {
                        // The texture uses SamplerWrapFunction::Repeat, so don't clamp to [0, 1].
                        // Also don't offset based on the polygon's bounds -- even if there are
                        // separate but adjacent polygons, we want seamless tiling.
                        let tx = pt.x() / tex_dims.width;
                        let ty = pt.y() / tex_dims.height;
                        [tx as f32, ty as f32, id.0, 100.0 + id.1]
                    }
                    Color::StretchTexture(id, _, angle) => {
                        // TODO Cache
                        let b = poly.get_bounds();
                        let center = poly.center();
                        let origin_pt = Pt2D::new(pt.x() - center.x(), pt.y() - center.y());
                        let (sin, cos) = angle.invert_y().normalized_radians().sin_cos();
                        let rot_pt = Pt2D::new(
                            center.x() + origin_pt.x() * cos - origin_pt.y() * sin,
                            center.y() + origin_pt.y() * cos + origin_pt.x() * sin,
                        );

                        let tx = (rot_pt.x() - b.min_x) / b.width();
                        let ty = (rot_pt.y() - b.min_y) / b.height();
                        [tx as f32, ty as f32, id.0, 100.0 + id.1]
                    }
                    // Two final special cases
                    Color::HatchingStyle1 => [100.0, 0.0, 0.0, 0.0],
                    Color::HatchingStyle2 => [101.0, 0.0, 0.0, 0.0],
                };
                vertices.push(Vertex {
                    position: [pt.x() as f32, pt.y() as f32],
                    style,
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

    pub fn upload_textures(
        &self,
        dims_to_textures: BTreeMap<(u32, u32), Vec<(String, Vec<u8>, TextureType)>>,
    ) {
        for (group_idx, (raw_dims, list)) in dims_to_textures.into_iter().enumerate() {
            let mut raw_data = Vec::new();
            for (tex_idx, (filename, raw, tex_type)) in list.into_iter().enumerate() {
                let tex_id = (group_idx as f32, tex_idx as f32);
                let dims = ScreenDims::new(f64::from(raw_dims.0), f64::from(raw_dims.1));
                self.texture_lookups.borrow_mut().insert(
                    filename,
                    match tex_type {
                        TextureType::Stretch => Color::StretchTexture(tex_id, dims, Angle::ZERO),
                        TextureType::Tile => Color::TileTexture(tex_id, dims),
                    },
                );
                raw_data.push(RawImage2d::from_raw_rgba(raw, raw_dims));
            }
            self.texture_arrays
                .borrow_mut()
                .push(Texture2dArray::new(&self.display, raw_data).unwrap());
        }
    }
}
