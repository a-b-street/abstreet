use crate::drawing::Uniforms;
use crate::{Canvas, Color, ScreenDims, ScreenRectangle, TextureType};
use geom::{Angle, Polygon, Pt2D};
use glow::HasContext;
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
        .with_depth_buffer(2)
        .build_windowed(window, &event_loop)
        .unwrap();
    let windowed_context = unsafe { context.make_current().unwrap() };
    let gl =
        glow::Context::from_loader_function(|s| windowed_context.get_proc_address(s) as *const _);

    unsafe {
        let program = gl.create_program().expect("Cannot create program");
        let shaders = [
            (glow::VERTEX_SHADER, include_str!("assets/vertex_140.glsl")),
            (
                glow::FRAGMENT_SHADER,
                include_str!("assets/fragment_140.glsl"),
            ),
        ]
        .into_iter()
        .map(|(shader_type, source)| {
            let shader = gl
                .create_shader(*shader_type)
                .expect("Cannot create shader");
            gl.shader_source(shader, source);
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                panic!(gl.get_shader_info_log(shader));
            }
            gl.attach_shader(program, shader);
            shader
        })
        .collect::<Vec<_>>();
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            panic!(gl.get_program_info_log(program));
        }
        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }
        gl.use_program(Some(program));
    }

    (
        PrerenderInnards {
            gl,
            windowed_context,
            total_bytes_uploaded: Cell::new(0),
            texture_lookups: RefCell::new(HashMap::new()),
        },
        event_loop,
    )
}

// Represents one frame that's gonna be drawn
pub struct GfxCtxInnards<'a> {
    gl: &'a glow::Context,
    windowed_context: &'a glutin::WindowedContext<glutin::PossiblyCurrent>,
}

impl<'a> GfxCtxInnards<'a> {
    pub fn clear(&mut self, color: Color) {
        match color {
            Color::RGBA(r, g, b, a) => unsafe {
                self.gl.clear_color(r, g, b, a);
                self.gl.clear(glow::COLOR_BUFFER_BIT);
            },
            _ => unreachable!(),
        }
    }

    pub fn redraw(&mut self, obj: &Drawable, uniforms: &Uniforms, prerender: &PrerenderInnards) {
        // TODO Uniforms

        unsafe {
            self.gl.bind_vertex_array(Some(obj.vert_array));
            self.gl.draw_arrays(glow::TRIANGLES, 0, obj.num_vertices);
            self.gl.bind_vertex_array(None);
        }

        /*self.target
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
        .unwrap();*/
    }

    pub fn enable_clipping(&mut self, rect: ScreenRectangle, canvas: &Canvas) {
        /*assert!(self.params.scissor.is_none());
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
        });*/
    }

    pub fn disable_clipping(&mut self) {
        /*assert!(self.params.scissor.is_some());
        self.params.scissor = None;*/
    }

    pub fn take_clip(&mut self) -> Option<bool> {
        //self.params.scissor.take()
        None
    }
    pub fn restore_clip(&mut self, clip: Option<bool>) {
        //self.params.scissor = clip;
    }

    pub fn finish(self) {
        self.windowed_context.swap_buffers().unwrap();
    }
}

// Something that's been sent to the GPU already.
pub struct Drawable {
    vert_buffer: u32,
    vert_array: u32,
    elem_buffer: u32,
    num_vertices: i32,
}

pub struct PrerenderInnards {
    gl: glow::Context,
    windowed_context: glutin::WindowedContext<glutin::PossiblyCurrent>,

    // TODO Prerender doesn't know what things are temporary and permanent. Could make the API more
    // detailed (and use the corresponding persistent glium types).
    pub total_bytes_uploaded: Cell<usize>,

    // Kind of a weird place for this, but ah well.
    pub(crate) texture_lookups: RefCell<HashMap<String, Color>>,
}

impl PrerenderInnards {
    pub fn actually_upload(&self, permanent: bool, list: Vec<(Color, &Polygon)>) -> Drawable {
        let mut vertices: Vec<[f32; 6]> = Vec::new();
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
                vertices.push([
                    pt.x() as f32,
                    pt.y() as f32,
                    style[0],
                    style[1],
                    style[2],
                    style[3],
                ]);
            }
            for idx in raw_indices {
                indices.push((idx_offset + *idx) as u32);
            }
        }

        let (vert_buffer, vert_array, elem_buffer) = unsafe {
            let vert_array = self.gl.create_vertex_array().unwrap();
            let vert_buffer = self.gl.create_buffer().unwrap();
            let elem_buffer = self.gl.create_buffer().unwrap();

            self.gl.bind_vertex_array(Some(vert_array));

            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(vert_buffer));
            self.gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                &vertices.align_to::<u8>().1,
                // TODO Use permanent
                glow::STATIC_DRAW,
            );

            self.gl
                .bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(elem_buffer));
            self.gl.buffer_data_u8_slice(
                glow::ELEMENT_ARRAY_BUFFER,
                &indices.align_to::<u8>().1,
                glow::STATIC_DRAW,
            );

            // TODO Can we have a single vertex array for everything, since there's an uber shader?

            // position... 2 f32's
            // TODO maybe need to do layout = 0 explicitly
            self.gl.enable_vertex_attrib_array(0);
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(vert_buffer)); // TODO why again?
                                                                        // TODO Can supposedly set 0 to auto-detect. Skip over 4 f32's of style to get to next
            let stride = 4 * std::mem::size_of::<f32>() as i32;
            self.gl
                .vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);

            // style... 4 f32's
            self.gl.enable_vertex_attrib_array(1);
            let stride = 2 * std::mem::size_of::<f32>() as i32;
            self.gl.vertex_attrib_pointer_f32(
                1,
                4,
                glow::FLOAT,
                false,
                stride,
                2 * std::mem::size_of::<f32>() as i32,
            );

            // Safety?
            self.gl.bind_vertex_array(None);
            self.gl.bind_buffer(glow::ARRAY_BUFFER, None);
            self.gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);

            (vert_buffer, vert_array, elem_buffer)
        };
        let num_vertices = vertices.len() as i32;

        if permanent {
            /*self.total_bytes_uploaded.set(
                self.total_bytes_uploaded.get()
                    + vertex_buffer.get_size()
                    + index_buffer.get_size(),
            );*/
        }

        Drawable {
            vert_buffer,
            vert_array,
            elem_buffer,
            num_vertices,
        }
    }

    pub fn request_redraw(&self) {
        self.windowed_context.window().request_redraw();
    }

    pub(crate) fn draw_new_frame(&self) -> GfxCtxInnards {
        GfxCtxInnards {
            gl: &self.gl,
            windowed_context: &self.windowed_context,
        }
    }

    pub(crate) fn upload_textures(
        &self,
        dims_to_textures: BTreeMap<(u32, u32), Vec<(String, Vec<u8>, TextureType)>>,
    ) {
        for (group_idx, (raw_dims, list)) in dims_to_textures.into_iter().enumerate() {
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
            }
        }
    }
}
