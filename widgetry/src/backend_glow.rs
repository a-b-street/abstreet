use std::cell::Cell;
use std::rc::Rc;

use glow::HasContext;

use crate::drawing::Uniforms;
use crate::{Canvas, Color, EventCtx, GeomBatch, ScreenDims, ScreenRectangle};

#[cfg(feature = "native-backend")]
pub use crate::backend_glow_native::setup;

#[cfg(feature = "wasm-backend")]
pub use crate::backend_glow_wasm::setup;

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

    pub fn take_clip(&mut self, scale_factor: f64, canvas: &Canvas) -> Option<[i32; 4]> {
        let clip = self.current_clip?;
        self.disable_clipping(scale_factor, canvas);
        Some(clip)
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

/// Geometry that's been uploaded to the GPU once and can be quickly redrawn many times. Create by
/// creating a `GeomBatch` and calling `ctx.upload(batch)`.
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

impl Drawable {
    /// This has no effect when drawn.
    pub fn empty(ctx: &EventCtx) -> Drawable {
        ctx.upload(GeomBatch::new())
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
type WindowAdapter = crate::backend_glow_wasm::WindowAdapter;

#[cfg(feature = "native-backend")]
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
        let mut vertices: Vec<[f32; 8]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for (color, poly, z) in batch.consume() {
            let idx_offset = vertices.len() as u32;
            let (pts, raw_indices) = poly.raw_for_rendering();
            for pt in pts {
                let style = color.shader_style(*pt);
                vertices.push([
                    pt.x() as f32,
                    pt.y() as f32,
                    z as f32,
                    style[0],
                    style[1],
                    style[2],
                    style[3],
                    style[4],
                ]);
            }
            for idx in raw_indices {
                indices.push(idx_offset + (*idx as u32));
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

            let vertex_attributes: [i32; 3] = [
                3, // position is vec2
                4, // color is vec4
                1, // texture_id is float
            ];
            let stride = vertex_attributes.iter().sum::<i32>() * std::mem::size_of::<f32>() as i32;
            let mut offset = 0;
            for (i, size) in vertex_attributes.iter().enumerate() {
                self.gl.enable_vertex_attrib_array(i as u32);
                self.gl.vertex_attrib_pointer_f32(
                    i as u32,
                    *size,
                    glow::FLOAT,
                    false,
                    stride,
                    offset,
                );
                offset += size * std::mem::size_of::<f32>() as i32;
            }

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

    pub(crate) fn screencap(&self, dims: ScreenDims, filename: String) -> anyhow::Result<()> {
        let width = dims.width as u32;
        let height = dims.height as u32;

        let mut img = image::DynamicImage::new_rgba8(width, height);
        let pixels = img.as_mut_rgba8().unwrap();

        unsafe {
            self.gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
            // TODO This starts at lower-left, I think we need to use window height here
            self.gl.read_pixels(
                0,
                0,
                width as i32,
                height as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(pixels),
            );
        }

        image::save_buffer(
            &filename,
            &image::imageops::flip_vertical(&img),
            width,
            height,
            image::ColorType::Rgba8,
        )?;
        Ok(())
    }
}

/// Uploads a sprite sheet of textures to the GPU so they can be used by Fill::Texture and
/// friends to paint shapes.
///
/// `path` - image file which is a grid of images.
/// `sprite_length` - the width and height of an individual cell in the image grid
///
/// The image file can have any number of sprites, but they must all be the same size.
///
/// Once uploaded, textures are addressed by their id, starting from 1, from left to right, top to
/// bottom, like so:
///
///   ┌─┬─┬─┐
///   │1│2│3│
///   ├─┼─┼─┤
///   │4│5│6│
///   ├─┼─┼─┤
///   │7│8│9│
///   └─┴─┴─┘
///
/// Texture(0) is reserved for a pure white (no-op) texture.
///
/// Implementation is based on the the description of ArrayTextures from:
/// https://www.khronos.org/opengl/wiki/Array_Texture.
pub fn load_textures(
    gl: &glow::Context,
    image_bytes: Vec<u8>,
    sprite_length: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let dynamic_img = image::load_from_memory(&image_bytes)?;
    let img = if let image::DynamicImage::ImageRgba8(img) = dynamic_img {
        img
    } else {
        todo!("support other image formats");
    };

    let format = glow::RGBA;
    let target = glow::TEXTURE_2D_ARRAY;
    let mipmap_level = 1;
    let internal_format = glow::RGBA;
    let bytes_per_pixel = 4;

    let texture_id = unsafe { gl.create_texture()? };
    unsafe {
        gl.bind_texture(target, Some(texture_id));
    }

    let (img_width, img_height) = img.dimensions();
    let sprite_height = sprite_length;
    let sprites_per_row = img_width / sprite_length;
    let sprites_per_column = img_height / sprite_length;
    let sprite_count = sprites_per_row * sprites_per_column;

    assert_eq!(
        sprites_per_row * sprite_length,
        img_width,
        "sprites must align exactly"
    );
    assert_eq!(
        sprites_per_column * sprite_height,
        img_height,
        "sprites must align exactly"
    );

    info!(
        "img_size: {}x{}px ({} px), sprite_size: {}x{}px, sprites: {}x{} ({} sprites)",
        img_width,
        img_height,
        img.pixels().len(),
        sprite_length,
        sprite_height,
        sprites_per_row,
        sprites_per_column,
        sprite_count
    );

    // Allocate the storage.
    unsafe {
        gl.tex_storage_3d(
            target,
            mipmap_level,
            internal_format,
            sprite_length as i32,
            sprite_height as i32,
            sprite_count as i32,
        );
    }

    // Upload pixel data.
    //
    // From: https://www.khronos.org/opengl/wiki/Array_Texture#Creation_and_Management
    // > The first 0 refers to the mipmap level (level 0, since there's only 1)
    // > The following 2 zeroes refers to the x and y offsets in case you only want to
    // > specify a subrectangle.
    // > The final 0 refers to the layer index offset (we start from index 0 and have 2
    // > levels).
    // > Altogether you can specify a 3D box subset of the overall texture, but only one
    // > mip level at a time.
    let mut formatted_pixel_bytes: Vec<u8> =
        Vec::with_capacity(img.pixels().len() * bytes_per_pixel);

    // In order to avoid branching in our shader logic, all shapes are rendered with a texture.
    // Even "non-textured" styles like Fill::Color, use a "default" no-op (pure white) texture,
    // which we generate here.
    formatted_pixel_bytes.append(&mut vec![
        255;
        (sprite_length * sprite_height) as usize
            * bytes_per_pixel
    ]);

    // OpenGL texture arrays expect each texture's bytes to be contiguous, but it's conventional to
    // store textures in a grid within a single spritesheet image, where a row and column traverses
    // multiple sprites.
    //
    // For example, if we had 6 textures, A-F, the input spritesheet bytes would be like:
    // [[AAA, BBB, CCC],
    //  [AAA, BBB, CCC]
    //  [AAA, BBB, CCC],
    //  [DDD, EEE, FFF],
    //  [DDD, EEE, FFF],
    //  [DDD, EEE, FFF]]
    //
    // Which we need to convert to:
    // [[AAAAAAAAA],
    //  [BBBBBBBBB],
    //  [CCCCCCCCC],
    //  [DDDDDDDDD],
    //  [EEEEEEEEE],
    //  [FFFFFFFFF]]
    use image::GenericImageView;
    for y in 0..sprites_per_column {
        for x in 0..sprites_per_row {
            let sprite_cell = img.view(
                x * sprite_length,
                y * sprite_height,
                sprite_length,
                sprite_height,
            );
            for p in sprite_cell.pixels() {
                formatted_pixel_bytes.extend_from_slice(&p.2 .0);
            }
        }
    }

    // prepare and generate mipmaps
    unsafe {
        gl.tex_sub_image_3d(
            target,
            0,
            0,
            0,
            0,
            sprite_length as i32,
            sprite_height as i32,
            sprite_count as i32,
            format,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(&formatted_pixel_bytes),
        );

        gl.tex_image_3d(
            target,
            0,
            format as i32,
            sprite_length as i32,
            sprite_height as i32,
            sprite_count as i32,
            0,
            format,
            glow::UNSIGNED_BYTE,
            Some(&formatted_pixel_bytes),
        );
        gl.tex_image_3d(
            target,
            1,
            format as i32,
            (sprite_length / 2) as i32,
            (sprite_height / 2) as i32,
            sprite_count as i32,
            0,
            format,
            glow::UNSIGNED_BYTE,
            Some(&formatted_pixel_bytes),
        );
        gl.tex_image_3d(
            target,
            2,
            format as i32,
            (sprite_length / 4) as i32,
            (sprite_height / 4) as i32,
            sprite_count as i32,
            0,
            format,
            glow::UNSIGNED_BYTE,
            Some(&formatted_pixel_bytes),
        );
        gl.generate_mipmap(target);
    }

    Ok(())
}
