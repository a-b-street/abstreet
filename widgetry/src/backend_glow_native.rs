pub use crate::backend_glow::Drawable;
use crate::backend_glow::{GfxCtxInnards, PrerenderInnards};
use crate::ScreenDims;
use glow::HasContext;

pub fn setup(window_title: &str) -> (PrerenderInnards, winit::event_loop::EventLoop<()>) {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title(window_title)
        .with_maximized(true);
    // TODO If people are hitting problems with context not matching what their GPU provides, dig up
    // backend_glium.rs from git and bring the fallback behavior here. (Ideally, there'd be
    // something in glutin to directly express this.) multisampling: 2 looks bad, 4 looks fine
    let context = glutin::ContextBuilder::new()
        .with_multisampling(4)
        .with_depth_buffer(2)
        .build_windowed(window, &event_loop)
        .unwrap();
    let windowed_context = unsafe { context.make_current().unwrap() };
    let gl = unsafe {
        glow::Context::from_loader_function(|s| windowed_context.get_proc_address(s) as *const _)
    };
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

    load_textures(&gl, "system/assets/textures/spritesheet.png", 64).unwrap();

    (
        PrerenderInnards::new(gl, program, WindowAdapter(windowed_context)),
        event_loop,
    )
}

pub struct WindowAdapter(glutin::WindowedContext<glutin::PossiblyCurrent>);

impl WindowAdapter {
    pub fn window(&self) -> &winit::window::Window {
        &self.0.window()
    }

    pub fn window_resized(&self, new_size: ScreenDims, scale_factor: f64) {
        let physical_size = winit::dpi::LogicalSize::from(new_size).to_physical(scale_factor);
        self.0.resize(physical_size);
    }

    pub fn draw_finished(&self, _gfc_ctx_innards: GfxCtxInnards) {
        self.0.swap_buffers().unwrap();
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
fn load_textures(
    gl: &glow::Context,
    filename: &str,
    sprite_length: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = abstutil::path(filename);
    let image_bytes = abstutil::slurp_file(&path)?;
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

    // In order to avoid branching in our shader logic, all shapes are rendered with a texture.
    // Even "non-textured" styles like Fill::Color, use a "default" no-op (pure white) texture,
    // which we generate here.
    let mut formatted_pixels: Vec<image::Rgba<u8>> =
        vec![image::Rgba([255; 4]); (sprite_length * sprite_length) as usize];

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
                formatted_pixels.push(p.2.clone());
            }
        }
    }

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
    let formatted_pixel_bytes: Vec<u8> =
        formatted_pixels.iter().flat_map(|p| p.0.to_vec()).collect();

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
