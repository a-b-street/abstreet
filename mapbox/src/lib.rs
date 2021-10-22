use wasm_bindgen::prelude::*;

use geom::Polygon;
use widgetry::{Color, RenderOnly};

#[wasm_bindgen]
pub struct PiggybackDemo {
    render_only: RenderOnly,
    //draw_something: Drawable,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl PiggybackDemo {
    pub fn create(gl: web_sys::WebGlRenderingContext) -> PiggybackDemo {
        // Use this to initialize logging.
        abstutil::CmdArgs::new().done();
        let render_only = RenderOnly::new(gl);
        PiggybackDemo { render_only }
    }

    pub fn draw(&self) {
        log::info!("Drawing...");
        let mut g = self.render_only.gfx_ctx();
        g.fork_screenspace();
        g.draw_polygon(Color::RED, Polygon::rectangle(50.0, 50.0));
        g.unfork();
    }
}
