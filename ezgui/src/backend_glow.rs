use crate::drawing::Uniforms;
use crate::{Canvas, Color, GeomBatch, ScreenDims, ScreenRectangle};
use glow::HasContext;
use std::cell::Cell;
use std::rc::Rc;

#[cfg(feature = "glow-backend")]
pub use crate::backend_glow_native::setup;

#[cfg(feature = "wasm-backend")]
pub use crate::backend_wasm::setup;

// Represents one frame that's gonna be drawn
pub struct GfxCtxInnards<'a> {
    gl: &'a glow::Context,
    program: &'a <glow::Context as glow::HasContext>::Program,
    current_clip: Option<[i32; 4]>,
}

impl<'a> GfxCtxInnards<'a> {
    pub fn new(
        gl: &'a glow::Context,
        program: &'a <glow::Context as glow::HasContext>::Program,
    ) -> Self {
        GfxCtxInnards {
            gl,
            program,
            current_clip: None,
        }
    }

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
                .uniform_3_f32_slice(Some(&transform_loc), &uniforms.transform);
            let window_loc = self
                .gl
                .get_uniform_location(*self.program, "window")
                .unwrap();
            self.gl
                .uniform_3_f32_slice(Some(&window_loc), &uniforms.window);

            self.gl.bind_vertex_array(Some(obj.vert_array.id));
            self.gl
                .draw_elements(glow::TRIANGLES, obj.num_indices, glow::UNSIGNED_INT, 0);
            self.gl.bind_vertex_array(None);
        }
    }

    pub fn enable_clipping(&mut self, rect: ScreenRectangle, scale_factor: f64, canvas: &Canvas) {
        assert!(self.current_clip.is_none());
        // The scissor rectangle is in units of physical pixles, as opposed to logical pixels
        let left = (rect.x1 * scale_factor) as i32;
        // Y-inversion
        let bottom = ((canvas.window_height - rect.y2) * scale_factor) as i32;
        let width = ((rect.x2 - rect.x1) * scale_factor) as i32;
        let height = ((rect.y2 - rect.y1) * scale_factor) as i32;
        unsafe {
            self.gl.scissor(left, bottom, width, height);
        }
        self.current_clip = Some([left, bottom, width, height]);
    }

    pub fn disable_clipping(&mut self, scale_factor: f64, canvas: &Canvas) {
        assert!(self.current_clip.is_some());
        self.current_clip = None;
        unsafe {
            self.gl.scissor(
                0,
                0,
                (canvas.window_width * scale_factor) as i32,
                (canvas.window_height * scale_factor) as i32,
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
}

// Something that's been sent to the GPU already.
pub struct Drawable {
    vert_array: VertexArray,
    vert_buffer: Buffer,
    elem_buffer: Buffer,
    num_indices: i32,
    gl: Rc<glow::Context>,
}

impl Drop for Drawable {
    #[inline]
    fn drop(&mut self) {
        self.elem_buffer.destroy(&self.gl);
        self.vert_buffer.destroy(&self.gl);
        self.vert_array.destroy(&self.gl);
    }
}

struct VertexArray {
    id: <glow::Context as glow::HasContext>::VertexArray,
    was_destroyed: bool,
}

impl VertexArray {
    fn new(gl: &glow::Context) -> VertexArray {
        let id = unsafe { gl.create_vertex_array().unwrap() };
        VertexArray {
            id,
            was_destroyed: false,
        }
    }

    fn destroy(&mut self, gl: &glow::Context) {
        assert!(!self.was_destroyed, "already destroyed");
        self.was_destroyed = true;
        unsafe {
            gl.delete_vertex_array(self.id);
        }
    }
}

impl Drop for VertexArray {
    fn drop(&mut self) {
        assert!(
            self.was_destroyed,
            "failed to call `destroy` before dropped. Memory leaked."
        );
    }
}

struct Buffer {
    id: <glow::Context as glow::HasContext>::Buffer,
    was_destroyed: bool,
}

impl Buffer {
    fn new(gl: &glow::Context) -> Buffer {
        let id = unsafe { gl.create_buffer().unwrap() };
        Buffer {
            id,
            was_destroyed: false,
        }
    }

    fn destroy(&mut self, gl: &glow::Context) {
        assert!(!self.was_destroyed, "already destroyed");
        self.was_destroyed = true;
        unsafe { gl.delete_buffer(self.id) };
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        assert!(
            self.was_destroyed,
            "failed to call `destroy` before dropped. Memory leaked."
        );
    }
}

#[cfg(feature = "wasm-backend")]
type WindowAdapter = crate::backend_wasm::WindowAdapter;

#[cfg(feature = "glow-backend")]
type WindowAdapter = crate::backend_glow_native::WindowAdapter;

pub struct PrerenderInnards {
    gl: Rc<glow::Context>,
    window_adapter: WindowAdapter,
    program: <glow::Context as glow::HasContext>::Program,

    // TODO Prerender doesn't know what things are temporary and permanent. Could make the API more
    // detailed.
    pub total_bytes_uploaded: Cell<usize>,
}

impl PrerenderInnards {
    pub fn new(
        gl: glow::Context,
        program: <glow::Context as glow::HasContext>::Program,
        window_adapter: WindowAdapter,
    ) -> PrerenderInnards {
        PrerenderInnards {
            gl: Rc::new(gl),
            program,
            window_adapter,
            total_bytes_uploaded: Cell::new(0),
        }
    }

    pub fn actually_upload(&self, permanent: bool, batch: GeomBatch) -> Drawable {
        let mut vertices: Vec<[f32; 6]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for (color, poly) in batch.consume() {
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
            let vert_array = VertexArray::new(&self.gl);
            let vert_buffer = Buffer::new(&self.gl);
            let elem_buffer = Buffer::new(&self.gl);

            self.gl.bind_vertex_array(Some(vert_array.id));

            self.gl
                .bind_buffer(glow::ARRAY_BUFFER, Some(vert_buffer.id));
            self.gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                &vertices.align_to::<u8>().1,
                // TODO Use permanent
                glow::STATIC_DRAW,
            );

            self.gl
                .bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(elem_buffer.id));
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
            vert_array,
            vert_buffer,
            elem_buffer,
            num_indices,
            gl: self.gl.clone(),
        }
    }

    fn window(&self) -> &winit::window::Window {
        self.window_adapter.window()
    }

    pub fn request_redraw(&self) {
        self.window().request_redraw();
    }

    pub fn set_cursor_icon(&self, icon: winit::window::CursorIcon) {
        self.window().set_cursor_icon(icon);
    }

    pub fn draw_new_frame(&self) -> GfxCtxInnards {
        GfxCtxInnards::new(&self.gl, &self.program)
    }

    pub fn window_resized(&self, new_size: ScreenDims, scale_factor: f64) {
        let physical_size = winit::dpi::LogicalSize::from(new_size).to_physical(scale_factor);
        self.window_adapter.window_resized(new_size, scale_factor);
        unsafe {
            self.gl
                .viewport(0, 0, physical_size.width, physical_size.height);
            // I think it's safe to assume there's not a clip right now.
            self.gl
                .scissor(0, 0, physical_size.width, physical_size.height);
        }
    }

    pub fn window_size(&self, scale_factor: f64) -> ScreenDims {
        self.window().inner_size().to_logical(scale_factor).into()
    }

    pub fn set_window_icon(&self, icon: winit::window::Icon) {
        self.window().set_window_icon(Some(icon));
    }

    pub fn monitor_scale_factor(&self) -> f64 {
        self.window().scale_factor()
    }

    pub fn draw_finished(&self, gfc_ctx_innards: GfxCtxInnards) {
        self.window_adapter.draw_finished(gfc_ctx_innards)
    }
}
