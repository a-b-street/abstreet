use crate::objects::{DrawCtx, ID};
use crate::plugins::{AmbientPlugin, PluginCtx};
use ezgui::{Color, GfxCtx, Key, Text};
use map_model::raw_data::StableRoadID;
use std::collections::BTreeMap;

pub struct DebugObjectsState {
    tooltip_key_held: bool,
    debug_tooltip_key_held: bool,
    selected: Option<ID>,
}

impl DebugObjectsState {
    pub fn new() -> DebugObjectsState {
        DebugObjectsState {
            tooltip_key_held: false,
            debug_tooltip_key_held: false,
            selected: None,
        }
    }
}

impl AmbientPlugin for DebugObjectsState {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        self.selected = ctx.primary.current_selection;
        if self.tooltip_key_held {
            self.tooltip_key_held = !ctx.input.key_released(Key::LeftControl);
        } else {
            // TODO Can't really display an OSD action if we're not currently selecting something.
            // Could only activate sometimes, but that seems a bit harder to use.
            self.tooltip_key_held = ctx
                .input
                .unimportant_key_pressed(Key::LeftControl, "hold to show tooltips");
        }
        if self.debug_tooltip_key_held {
            self.debug_tooltip_key_held = !ctx.input.key_released(Key::RightControl);
        } else {
            self.debug_tooltip_key_held = ctx
                .input
                .unimportant_key_pressed(Key::RightControl, "hold to show debug tooltips");
        }

        if let Some(id) = self.selected {
            if ctx.input.contextual_action(Key::D, "debug") {
                id.debug(
                    &ctx.primary.map,
                    &mut ctx.primary.sim,
                    &ctx.primary.draw_map,
                );
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        if self.tooltip_key_held {
            if let Some(id) = self.selected {
                let txt = tooltip_lines(id, g, ctx);
                g.draw_mouse_tooltip(txt);
            }
        }

        if self.debug_tooltip_key_held {
            if let Some(pt) = g.canvas.get_cursor_in_map_space() {
                if let Some(gps) = pt.to_gps(ctx.map.get_gps_bounds()) {
                    let mut txt = Text::new();
                    txt.add_line(format!("{}", pt));
                    txt.add_line(format!("{}", gps));
                    g.draw_mouse_tooltip(txt);
                }
            }
        }
    }
}

fn tooltip_lines(obj: ID, g: &mut GfxCtx, ctx: &DrawCtx) -> Text {
    let (map, sim, draw_map) = (&ctx.map, &ctx.sim, &ctx.draw_map);
    let mut txt = Text::new();
    match obj {
        ID::Road(id) => {
            let r = map.get_r(id);
            txt.add_line(format!("{} (originally {}) is ", r.id, r.stable_id));
            txt.append(
                r.osm_tags
                    .get("name")
                    .unwrap_or(&"???".to_string())
                    .to_string(),
                Some(Color::CYAN),
            );
            txt.add_line(format!("From OSM way {}", r.osm_way_id));
        }
        ID::Lane(id) => {
            let l = map.get_l(id);
            let r = map.get_r(l.parent);
            let i1 = map.get_source_intersection(id);
            let i2 = map.get_destination_intersection(id);

            txt.add_line(format!("{} is ", l.id));
            txt.append(
                r.osm_tags
                    .get("name")
                    .unwrap_or(&"???".to_string())
                    .to_string(),
                Some(Color::CYAN),
            );
            txt.add_line(format!("From OSM way {}", r.osm_way_id));
            txt.add_line(format!(
                "Parent {} (originally {}) points to {}",
                r.id, r.stable_id, r.dst_i
            ));
            txt.add_line(format!(
                "Lane goes from {} to {}",
                i1.elevation, i2.elevation
            ));
            txt.add_line(format!(
                "Lane is {} long, parent {} is {} long",
                l.length(),
                r.id,
                r.center_pts.length()
            ));
            styled_kv(&mut txt, &r.osm_tags);
            if l.is_parking() {
                txt.add_line(format!("Has {} parking spots", l.number_parking_spots()));
            }
        }
        ID::Intersection(id) => {
            txt.add_line(id.to_string());
            let i = map.get_i(id);
            txt.add_line(format!("Roads: {:?}", i.roads));
            txt.add_line(format!(
                "Orig roads: {:?}",
                i.roads
                    .iter()
                    .map(|r| map.get_r(*r).stable_id)
                    .collect::<Vec<StableRoadID>>()
            ));
            txt.add_line(format!("Originally {}", i.stable_id));
        }
        ID::Turn(id) => {
            let t = map.get_t(id);
            txt.add_line(format!("{}", id));
            txt.add_line(format!("{:?}", t.turn_type));
        }
        ID::Building(id) => {
            let b = map.get_b(id);
            txt.add_line(format!(
                "Building #{:?} (from OSM way {})",
                id, b.osm_way_id
            ));
            txt.add_line(format!(
                "Dist along sidewalk: {}",
                b.front_path.sidewalk.dist_along()
            ));
            if let Some(units) = b.num_residential_units {
                txt.add_line(format!("{} residential units", units));
            }
            styled_kv(&mut txt, &b.osm_tags);
        }
        ID::Car(id) => {
            for line in sim.car_tooltip(id) {
                txt.add_wrapped_line(&g.canvas, line);
            }
        }
        ID::Pedestrian(id) => {
            for line in sim.ped_tooltip(id) {
                txt.add_wrapped_line(&g.canvas, line);
            }
        }
        ID::ExtraShape(id) => {
            styled_kv(&mut txt, &draw_map.get_es(id).attributes);
        }
        ID::Parcel(id) => {
            txt.add_line(id.to_string());
        }
        ID::BusStop(id) => {
            txt.add_line(id.to_string());
            for r in map.get_all_bus_routes() {
                if r.stops.contains(&id) {
                    txt.add_line(format!("- Route {}", r.name));
                }
            }
        }
        ID::Area(id) => {
            let a = map.get_a(id);
            txt.add_line(format!("{} (from OSM {})", id, a.osm_id));
            styled_kv(&mut txt, &a.osm_tags);
        }
        ID::Trip(_) => {}
    };
    txt
}

fn styled_kv(txt: &mut Text, tags: &BTreeMap<String, String>) {
    for (k, v) in tags {
        txt.add_styled_line(k.to_string(), Some(Color::RED), None);
        txt.append(" = ".to_string(), None);
        txt.append(v.to_string(), Some(Color::CYAN));
    }
}
