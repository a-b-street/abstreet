#[macro_use]
extern crate log;

use wasm_bindgen::prelude::*;

use abstutil::Timer;
use geom::{LonLat, Pt2D, Time};
use map_gui::colors::ColorScheme;
use map_gui::options::Options;
use map_gui::render::{DrawMap, DrawOptions};
use map_gui::{AppLike, ID};
use map_model::Map;
use widgetry::{EventCtx, GfxCtx, RenderOnly, Settings, State};

#[wasm_bindgen]
pub struct PiggybackDemo {
    render_only: RenderOnly,
    map: Map,
    draw_map: DrawMap,
    cs: ColorScheme,
    options: Options,
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

        let mut render_only = RenderOnly::new(
            gl,
            Settings::new("Piggyback demo").read_svg(Box::new(abstio::slurp_bytes)),
        );

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
            options,
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

    pub fn draw_zoomed(&self) {
        let g = &mut self.render_only.gfx_ctx();

        let objects = self
            .draw_map
            .get_renderables_back_to_front(g.get_screen_bounds(), &self.map);

        let opts = DrawOptions::new();
        for obj in objects {
            if matches!(
                obj.get_id(),
                ID::Lane(_) | ID::Intersection(_) | ID::Road(_)
            ) {
                obj.draw(g, self, &opts);
            }
        }
    }
}

// Drawing some of the objects requires this interface. The unreachable methods should, as the name
// suggests, not actually get called
impl AppLike for PiggybackDemo {
    fn map(&self) -> &Map {
        &self.map
    }
    fn sim(&self) -> &sim::Sim {
        unreachable!()
    }
    fn cs(&self) -> &ColorScheme {
        &self.cs
    }
    fn mut_cs(&mut self) -> &mut ColorScheme {
        &mut self.cs
    }
    fn draw_map(&self) -> &DrawMap {
        &self.draw_map
    }
    fn mut_draw_map(&mut self) -> &mut DrawMap {
        &mut self.draw_map
    }
    fn opts(&self) -> &Options {
        &self.options
    }
    fn mut_opts(&mut self) -> &mut Options {
        &mut self.options
    }
    fn map_switched(&mut self, _: &mut EventCtx, _: map_model::Map, _: &mut abstutil::Timer) {
        unreachable!()
    }
    fn draw_with_opts(&self, _: &mut GfxCtx, _: map_gui::render::DrawOptions) {
        unreachable!()
    }
    fn make_warper(
        &mut self,
        _: &EventCtx,
        _: Pt2D,
        _: Option<f64>,
        _: Option<map_gui::ID>,
    ) -> Box<dyn State<PiggybackDemo>> {
        unreachable!()
    }
    fn sim_time(&self) -> Time {
        Time::START_OF_DAY
    }
    fn current_stage_and_remaining_time(
        &self,
        i: map_model::IntersectionID,
    ) -> (usize, geom::Duration) {
        (
            0,
            self.map.get_traffic_signal(i).stages[0]
                .stage_type
                .simple_duration(),
        )
    }
}
