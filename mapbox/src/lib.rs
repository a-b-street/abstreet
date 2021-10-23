#[macro_use]
extern crate log;

use wasm_bindgen::prelude::*;

use abstutil::Timer;
use geom::{LonLat, Pt2D};
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

    pub fn move_canvas(&mut self, ne_lon: f64, ne_lat: f64, sw_lon: f64, sw_lat: f64) {
        let gps_bounds = self.map.get_gps_bounds();
        let top_left = LonLat::new(ne_lon, ne_lat).to_pt(gps_bounds);
        let bottom_right = LonLat::new(sw_lon, sw_lat).to_pt(gps_bounds);
        let center =
            LonLat::new((ne_lon + sw_lon) / 2.0, (ne_lat + sw_lat) / 2.0).to_pt(gps_bounds);

        let mut ctx = self.render_only.event_ctx();
        // This is quite a strange way of calculating zoom
        let want_diagonal_dist = top_left.dist_to(bottom_right);
        let b = ctx.canvas.get_screen_bounds();
        let current_diagonal_dist =
            Pt2D::new(b.min_x, b.min_y).dist_to(Pt2D::new(b.max_x, b.max_y));
        // We can do this calculation before changing the center, because we're working in mercator
        // already

        ctx.canvas.cam_zoom *= current_diagonal_dist / want_diagonal_dist;
        ctx.canvas.center_on_map_pt(center);
    }

    pub fn draw(&self) {
        let mut g = self.render_only.gfx_ctx();

        //g.clear(self.cs.void_background);
        //g.redraw(&self.draw_map.boundary_polygon);
        //g.redraw(&self.draw_map.draw_all_areas);
        //g.redraw(&self.draw_map.draw_all_unzoomed_parking_lots);
        g.redraw(&self.draw_map.draw_all_unzoomed_roads_and_intersections);
        //g.redraw(&self.draw_map.draw_all_buildings);
        //g.redraw(&self.draw_map.draw_all_building_outlines);
    }
}
