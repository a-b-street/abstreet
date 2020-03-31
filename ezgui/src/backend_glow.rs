use crate::drawing::Uniforms;
use crate::{Canvas, Color, FancyColor, ScreenDims, ScreenRectangle};
use geom::Polygon;
use glow::HasContext;
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
        .with_depth_buffer(2)
        .build_windowed(window, &event_loop)
        .unwrap();
    let windowed_context = unsafe { context.make_current().unwrap() };
    let gl =
        glow::Context::from_loader_function(|s| windowed_context.get_proc_address(s) as *const _);
    let program = unsafe { gl.create_program().expect("Cannot create program") };

    unsafe {
        let shaders = [
            (glow::VERTEX_SHADER, include_str!("shaders/vertex_140.glsl")),
            (
                glow::FRAGMENT_SHADER,
                include_str!("shaders/fragment_140.glsl"),
            ),
        ]
        .iter()
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

        gl.enable(glow::SCISSOR_TEST);

        gl.enable(glow::DEPTH_TEST);
        gl.depth_func(glow::LEQUAL);

        gl.enable(glow::BLEND);
        gl.blend_func_separate(
            glow::SRC_ALPHA,
            glow::ONE_MINUS_SRC_ALPHA,
            glow::SRC_ALPHA,
            glow::ONE_MINUS_SRC_ALPHA,
        );
    }

    let window_size = event_loop.primary_monitor().size();
    (
        PrerenderInnards {
            gl,
            program,
            windowed_context,
            total_bytes_uploaded: Cell::new(0),
        },
        event_loop,
        ScreenDims::new(window_size.width.into(), window_size.height.into()),
    )
}

// Represents one frame that's gonna be drawn
pub struct GfxCtxInnards<'a> {
    gl: &'a glow::Context,
    windowed_context: &'a glutin::WindowedContext<glutin::PossiblyCurrent>,
    program: &'a <glow::Context as glow::HasContext>::Program,

    current_clip: Option<[i32; 4]>,
}

impl<'a> GfxCtxInnards<'a> {
    pub fn clear(&mut self, color: Color) {
        unsafe {
            self.gl.clear_color(color.r, color.g, color.b, color.a);
            self.gl.clear(glow::COLOR_BUFFER_BIT);

            self.gl.clear_depth_f32(1.0);
            self.gl.clear(glow::DEPTH_BUFFER_BIT);
        }
    }

    pub fn redraw(&mut self, obj: &Drawable, uniforms: &Uniforms, _: &PrerenderInnards) {
        unsafe {
            let transform_loc = self
                .gl
                .get_uniform_location(*self.program, "transform")
                .unwrap();
            self.gl
                .uniform_3_f32_slice(Some(transform_loc), &uniforms.transform);
            let window_loc = self
                .gl
                .get_uniform_location(*self.program, "window")
                .unwrap();
            self.gl
                .uniform_3_f32_slice(Some(window_loc), &uniforms.window);

            self.gl.bind_vertex_array(Some(obj.vert_array));
            self.gl
                .draw_elements(glow::TRIANGLES, obj.num_indices, glow::UNSIGNED_INT, 0);
            self.gl.bind_vertex_array(None);
        }
    }

    pub fn enable_clipping(&mut self, rect: ScreenRectangle, canvas: &Canvas) {
        assert!(self.current_clip.is_none());
        // The scissor rectangle has to be in device coordinates, so you would think some transform
        // by scale factor (previously called HiDPI factor) has to happen here. But actually,
        // window dimensions and the rectangle passed in are already scaled up. So don't do
        // anything here!
        let left = rect.x1 as i32;
        // Y-inversion
        let bottom = (canvas.window_height - rect.y2) as i32;
        let width = (rect.x2 - rect.x1) as i32;
        let height = (rect.y2 - rect.y1) as i32;
        unsafe {
            self.gl.scissor(left, bottom, width, height);
        }
        self.current_clip = Some([left, bottom, width, height]);
    }

    pub fn disable_clipping(&mut self, canvas: &Canvas) {
        assert!(self.current_clip.is_some());
        self.current_clip = None;
        unsafe {
            self.gl.scissor(
                0,
                0,
                canvas.window_width as i32,
                canvas.window_height as i32,
            );
        }
    }

    pub fn take_clip(&mut self) -> Option<[i32; 4]> {
        self.current_clip.take()
    }
    pub fn restore_clip(&mut self, clip: Option<[i32; 4]>) {
        self.current_clip = clip;
        if let Some(c) = clip {
            unsafe {
                self.gl.scissor(c[0], c[1], c[2], c[3]);
            }
        }
    }

    pub fn finish(self) {
        self.windowed_context.swap_buffers().unwrap();
    }
}

// Something that's been sent to the GPU already.
// TODO Implement Drop; have to keep a reference to gl.
pub struct Drawable {
    _vert_buffer: u32,
    vert_array: u32,
    _elem_buffer: u32,
    num_indices: i32,
}

pub struct PrerenderInnards {
    gl: glow::Context,
    windowed_context: glutin::WindowedContext<glutin::PossiblyCurrent>,
    program: <glow::Context as glow::HasContext>::Program,

    // TODO Prerender doesn't know what things are temporary and permanent. Could make the API more
    // detailed.
    pub total_bytes_uploaded: Cell<usize>,
}

impl PrerenderInnards {
    pub fn actually_upload(&self, permanent: bool, list: Vec<(FancyColor, &Polygon)>) -> Drawable {
        let mut vertices: Vec<[f32; 6]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for (color, poly) in list {
            let idx_offset = vertices.len();
            let (pts, raw_indices) = poly.raw_for_rendering();
            for pt in pts {
                let style = color.style(*pt);
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

            let stride = 6 * std::mem::size_of::<f32>() as i32;
            // position is vec2
            self.gl.enable_vertex_attrib_array(0);
            self.gl
                .vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
            // style is vec4
            self.gl.enable_vertex_attrib_array(1);
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
        let num_indices = indices.len() as i32;

        if permanent {
            /*self.total_bytes_uploaded.set(
                self.total_bytes_uploaded.get()
                    + vertex_buffer.get_size()
                    + index_buffer.get_size(),
            );*/
        }

        Drawable {
            _vert_buffer: vert_buffer,
            vert_array,
            _elem_buffer: elem_buffer,
            num_indices,
        }
    }

    pub fn request_redraw(&self) {
        self.windowed_context.window().request_redraw();
    }

    pub fn draw_new_frame(&self) -> GfxCtxInnards {
        GfxCtxInnards {
            gl: &self.gl,
            windowed_context: &self.windowed_context,
            program: &self.program,
            current_clip: None,
        }
    }

    pub fn window_resized(&self, width: f64, height: f64) {
        self.windowed_context
            .resize(winit::dpi::PhysicalSize::new(width as u32, height as u32));
        unsafe {
            self.gl.viewport(0, 0, width as i32, height as i32);
            // I think it's safe to assume there's not a clip right now.
            self.gl.scissor(0, 0, width as i32, height as i32);
        }
    }

    pub fn monitor_scale_factor(&self) -> f64 {
        self.windowed_context.window().scale_factor()
    }
}
