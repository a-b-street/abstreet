use ezgui::{Color, GfxCtx, Text, TEXT_FG_COLOR};
use map_model::Map;
use objects::{Ctx, ID};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use render::DrawMap;
use sim::Sim;

pub enum DebugObjectsState {
    Empty,
    Selected(ID),
    Tooltip(ID),
}

impl DebugObjectsState {
    pub fn new() -> DebugObjectsState {
        DebugObjectsState::Empty
    }
}

impl Plugin for DebugObjectsState {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let new_state = if let Some(id) = ctx.primary.current_selection {
            // Don't break out of the tooltip state
            if let DebugObjectsState::Tooltip(_) = self {
                DebugObjectsState::Tooltip(id)
            } else {
                DebugObjectsState::Selected(id)
            }
        } else {
            DebugObjectsState::Empty
        };
        *self = new_state;

        let mut new_state: Option<DebugObjectsState> = None;
        match self {
            DebugObjectsState::Empty => {}
            DebugObjectsState::Selected(id) => {
                if ctx
                    .input
                    .key_pressed(Key::LCtrl, &format!("Hold Ctrl to show {:?}'s tooltip", id))
                {
                    new_state = Some(DebugObjectsState::Tooltip(*id));
                } else if ctx.input.key_pressed(Key::D, "debug") {
                    id.debug(
                        &ctx.primary.map,
                        &mut ctx.primary.sim,
                        &ctx.primary.draw_map,
                    );
                }
            }
            DebugObjectsState::Tooltip(id) => {
                if ctx.input.key_released(Key::LCtrl) {
                    new_state = Some(DebugObjectsState::Selected(*id));
                }
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            DebugObjectsState::Empty => false,
            // TODO hmm, but when we press D to debug, we don't want other stuff to happen...
            DebugObjectsState::Selected(_) => false,
            DebugObjectsState::Tooltip(_) => true,
        }
    }

    fn draw(&self, g: &mut GfxCtx, ctx: Ctx) {
        match *self {
            DebugObjectsState::Empty => {}
            DebugObjectsState::Selected(_) => {}
            DebugObjectsState::Tooltip(id) => {
                ctx.canvas
                    .draw_mouse_tooltip(g, tooltip_lines(id, ctx.map, ctx.sim, ctx.draw_map));
            }
        }
    }
}

fn tooltip_lines(obj: ID, map: &Map, sim: &Sim, draw_map: &DrawMap) -> Text {
    let mut txt = Text::new();
    match obj {
        ID::Lane(id) => {
            let l = map.get_l(id);
            let r = map.get_r(l.parent);
            let i1 = map.get_source_intersection(id);
            let i2 = map.get_destination_intersection(id);

            txt.add_line(format!(
                "{} is {}",
                l.id,
                r.osm_tags.get("name").unwrap_or(&"???".to_string())
            ));
            txt.add_line(format!("From OSM way {}", r.osm_way_id));
            txt.add_line(format!("Parent {} points to {}", r.id, r.dst_i));
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
            for (k, v) in &r.osm_tags {
                txt.add_line(format!("{} = {}", k, v));
            }
            if l.is_parking() {
                txt.add_line(format!("Has {} parking spots", l.number_parking_spots()));
            }
        }
        ID::Intersection(id) => {
            txt.add_line(id.to_string());
            txt.add_line(format!("Roads: {:?}", map.get_i(id).roads));
        }
        ID::Turn(id) => {
            let t = map.get_t(id);
            txt.add_line(format!("{}", id));
            txt.add_line(format!("{:?} / {:?}", t.turn_type, t.turn_angle(map)));
        }
        ID::Building(id) => {
            let b = map.get_b(id);
            txt.add_line(format!(
                "Building #{:?} (from OSM way {})",
                id, b.osm_way_id
            ));
            for (k, v) in &b.osm_tags {
                txt.add_styled_line(k.to_string(), Color::RED, None);
                txt.append(" = ".to_string(), TEXT_FG_COLOR, None);
                txt.append(v.to_string(), Color::BLUE, None);
            }
        }
        ID::Car(id) => {
            for line in sim.car_tooltip(id) {
                txt.add_line(line);
            }
        }
        ID::Pedestrian(id) => {
            for line in sim.ped_tooltip(id) {
                txt.add_line(line);
            }
        }
        ID::ExtraShape(id) => {
            for (k, v) in &draw_map.get_es(id).attributes {
                txt.add_styled_line(k.to_string(), Color::RED, None);
                txt.append(" = ".to_string(), TEXT_FG_COLOR, None);
                txt.append(v.to_string(), Color::BLUE, None);
            }
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
            txt.add_line(format!("{} (from OSM way {})", id, a.osm_way_id));
            for (k, v) in &a.osm_tags {
                txt.add_line(format!("{} = {}", k, v));
            }
        }
        ID::Trip(_) => {}
    };
    txt
}
