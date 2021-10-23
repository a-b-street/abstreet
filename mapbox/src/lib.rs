#[macro_use]
extern crate log;

use wasm_bindgen::prelude::*;

use abstutil::Timer;
use geom::Polygon;
use map_gui::colors::ColorScheme;
use map_gui::render::DrawMap;
use map_model::Map;
use widgetry::{Color, RenderOnly};

#[wasm_bindgen]
pub struct PiggybackDemo {
    render_only: RenderOnly,
    map: Map,
    draw_map: DrawMap,
    cs: ColorScheme,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl PiggybackDemo {
    pub fn create_with_map_bytes(
        gl: web_sys::WebGlRenderingContext,
        bytes: js_sys::ArrayBuffer,
    ) -> PiggybackDemo {
        // Use this to initialize logging.
        abstutil::CmdArgs::new().done();

        let mut render_only = RenderOnly::new(gl);

        // This is convoluted and it works
        let array = js_sys::Uint8Array::new(&bytes);
        info!("Parsing {} map bytes", bytes.byte_length());
        let mut timer = Timer::new("loading map");
        let mut map: Map = abstutil::from_binary(&array.to_vec()).unwrap();
        map.map_loaded_directly(&mut timer);
        info!("loaded {:?}", map.get_name());

        let mut ctx = render_only.event_ctx();
        let cs = ColorScheme::new(&mut ctx, map_gui::colors::ColorSchemeChoice::DayMode);
        let options = map_gui::options::Options::load_or_default();
        info!("making draw map");
        let draw_map = DrawMap::new(&mut ctx, &map, &options, &cs, &mut timer);

        PiggybackDemo {
            render_only,
            map,
            draw_map,
            cs,
        }
    }

    pub fn draw(&self) {
        log::info!("Drawing...");
        let mut g = self.render_only.gfx_ctx();

        //g.clear(self.cs.void_background);
        //g.redraw(&self.draw_map.boundary_polygon);
        //g.redraw(&self.draw_map.draw_all_areas);
        g.redraw(&self.draw_map.draw_all_unzoomed_parking_lots);
        g.redraw(&self.draw_map.draw_all_unzoomed_roads_and_intersections);
        //g.redraw(&self.draw_map.draw_all_buildings);
        //g.redraw(&self.draw_map.draw_all_building_outlines);
    }
}
