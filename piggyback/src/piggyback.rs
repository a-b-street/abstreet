use wasm_bindgen::prelude::*;

use abstutil::{prettyprint_usize, Timer};
use geom::{Circle, Distance, Duration, LonLat, Pt2D, Time};
use map_gui::colors::ColorScheme;
use map_gui::options::Options;
use map_gui::render::{AgentCache, DrawMap, DrawOptions};
use map_gui::{AppLike, ID};
use map_model::{Map, Traversable};
use sim::Sim;
use widgetry::{EventCtx, GfxCtx, RenderOnly, Settings, State};

/// This allows part of A/B Street to "piggyback" onto a WebGL canvas managed by something else,
/// such as Mapbox GL.
#[wasm_bindgen]
pub struct PiggybackDemo {
    render_only: RenderOnly,
    map: Map,
    sim: Sim,
    draw_map: DrawMap,
    agents: AgentCache,
    cs: ColorScheme,
    options: Options,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl PiggybackDemo {
    /// Initializes the piggyback mode with a WebGL context and raw bytes representing a map file
    /// to manage. (The map file shouldn't be gzipped.)
    pub fn create_with_map_bytes(
        gl: web_sys::WebGlRenderingContext,
        map_bytes: js_sys::ArrayBuffer,
    ) -> PiggybackDemo {
        abstutil::logger::setup();

        let mut render_only = RenderOnly::new(
            gl,
            Settings::new("Piggyback demo").read_svg(Box::new(abstio::slurp_bytes)),
        );

        let mut timer = Timer::new("loading map");
        let array = js_sys::Uint8Array::new(&map_bytes);
        info!(
            "Parsing {} map bytes",
            prettyprint_usize(map_bytes.byte_length() as usize)
        );
        let mut map: Map = abstutil::from_binary(&array.to_vec()).unwrap();
        map.map_loaded_directly(&mut timer);
        info!("Loaded {:?}", map.get_name());

        let sim = Sim::new(&map, sim::SimOptions::default());

        let mut ctx = render_only.event_ctx();
        let cs = ColorScheme::new(&mut ctx, map_gui::colors::ColorSchemeChoice::DayMode);
        let options = map_gui::options::Options::load_or_default();
        info!("Creating draw map");
        let draw_map = DrawMap::new(&mut ctx, &map, &options, &cs, &mut timer);

        PiggybackDemo {
            render_only,
            map,
            sim,
            draw_map,
            agents: AgentCache::new(),
            cs,
            options,
        }
    }

    /// Set the camera to match a northeast and southwest corner, given by lon/lat.
    pub fn move_canvas(&mut self, ne_lon: f64, ne_lat: f64, sw_lon: f64, sw_lat: f64) {
        let gps_bounds = self.map.get_gps_bounds();
        let top_left = LonLat::new(ne_lon, ne_lat).to_pt(gps_bounds);
        let bottom_right = LonLat::new(sw_lon, sw_lat).to_pt(gps_bounds);
        let center =
            LonLat::new((ne_lon + sw_lon) / 2.0, (ne_lat + sw_lat) / 2.0).to_pt(gps_bounds);

        let mut ctx = self.render_only.event_ctx();
        // This is quite a strange way of calculating zoom, but it works
        let want_diagonal_dist = top_left.dist_to(bottom_right);
        let b = ctx.canvas.get_screen_bounds();
        let current_diagonal_dist =
            Pt2D::new(b.min_x, b.min_y).dist_to(Pt2D::new(b.max_x, b.max_y));
        // We can do this calculation before changing the center, because we're working in mercator
        // already; distances shouldn't change based on where we are.

        ctx.canvas.cam_zoom *= current_diagonal_dist / want_diagonal_dist;
        ctx.canvas.center_on_map_pt(center);
    }

    /// Advances the traffic simulation.
    pub fn advance_sim_time(&mut self, delta_milliseconds: f64) {
        let dt = Duration::milliseconds(delta_milliseconds);
        // Use the real time passed as the deadline
        self.sim.time_limited_step(&self.map, dt, dt, &mut None);
    }

    /// Spawn random, unrealistic traffic.
    pub fn spawn_traffic(&mut self) {
        let mut rng = sim::SimFlags::for_test("spawn_traffic").make_rng();
        let mut timer = Timer::new("spawn traffic");
        sim::ScenarioGenerator::small_run(&self.map)
            .generate(&self.map, &mut rng, &mut timer)
            .instantiate(&mut self.sim, &self.map, &mut rng, &mut timer);
    }

    /// Reset the traffic simulation.
    pub fn clear_traffic(&mut self) {
        self.sim = Sim::new(&self.map, sim::SimOptions::default());
    }

    /// Draw the zoomed-in view. If `show_roads` is true, render roads and intersections in detail.
    /// Always draw agents in their zoomed-in view. Don't draw anything else -- it's assumed that
    /// the web app otherwise renders areas, buildings, and such already.
    // Note this is &mut to conveniently work with AgentCache. Other code uses RefCell.
    pub fn draw_zoomed(&mut self, show_roads: bool) {
        // Short-circuit if there's nothing to do
        if !show_roads && self.sim.is_empty() {
            return;
        }

        let g = &mut self.render_only.gfx_ctx();

        let objects = self
            .draw_map
            .get_renderables_back_to_front(g.get_screen_bounds(), &self.map);

        let opts = DrawOptions::new();
        // As we draw the static map elements, track where we need to draw live agents.
        let mut agents_on = Vec::new();
        for obj in objects {
            if show_roads {
                if let ID::Lane(_) | ID::Intersection(_) | ID::Road(_) = obj.get_id() {
                    obj.draw(g, self, &opts);
                }
            }

            if let ID::Lane(l) = obj.get_id() {
                agents_on.push(Traversable::Lane(l));
            }
            if let ID::Intersection(i) = obj.get_id() {
                for t in &self.map.get_i(i).turns {
                    agents_on.push(Traversable::Turn(t.id));
                }
            }
        }

        for on in agents_on {
            self.agents
                .populate_if_needed(on, &self.map, &self.sim, &self.cs, g.prerender);
            for obj in self.agents.get(on) {
                obj.draw(g, self, &opts);
            }
        }
    }

    /// Draw unzoomed agents.
    pub fn draw_unzoomed(&mut self) {
        if self.sim.is_empty() {
            return;
        }
        let g = &mut self.render_only.gfx_ctx();
        self.agents
            .draw_unzoomed_agents(g, &self.map, &self.sim, &self.cs, &self.options);
    }

    /// If there's a road, intersection, or area at the specififed coordinates, return a JSON
    /// string with debug info.
    pub fn debug_object_at(&self, lon: f64, lat: f64) -> Option<String> {
        let pt = LonLat::new(lon, lat).to_pt(self.map.get_gps_bounds());
        let mut objects = self.draw_map.get_renderables_back_to_front(
            Circle::new(pt, Distance::meters(3.0)).get_bounds(),
            &self.map,
        );
        objects.reverse();
        for obj in objects {
            if obj.contains_pt(pt, &self.map) {
                let json = match obj.get_id() {
                    ID::Road(r) => abstutil::to_json(self.map.get_r(r)),
                    ID::Intersection(i) => abstutil::to_json(self.map.get_i(i)),
                    ID::Area(a) => abstutil::to_json(self.map.get_a(a)),
                    _ => continue,
                };
                return Some(json);
            }
        }
        None
    }
}

// Drawing some of the objects requires this interface
impl AppLike for PiggybackDemo {
    fn map(&self) -> &Map {
        &self.map
    }
    fn sim(&self) -> &Sim {
        &self.sim
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
        self.sim.time()
    }
}
