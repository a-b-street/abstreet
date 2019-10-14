use crate::common::CommonState;
use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::ui::UI;
use ezgui::{hotkey, Color, EventCtx, GfxCtx, Key, Line, ModalMenu, Text};
use std::collections::BTreeMap;

pub struct InfoPanel {
    txt: Text,
    menu: ModalMenu,
}

impl InfoPanel {
    pub fn new(id: ID, ui: &UI, ctx: &EventCtx) -> InfoPanel {
        InfoPanel {
            txt: info_for(id, ui, ctx),
            menu: ModalMenu::new("Info Panel", vec![vec![(hotkey(Key::Escape), "quit")]], ctx),
        }
    }
}

impl State for InfoPanel {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut UI) -> Transition {
        self.menu.event(ctx);
        if self.menu.action("quit") {
            Transition::Pop
        } else {
            Transition::Keep
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        g.draw_blocking_text(
            &self.txt,
            (
                ezgui::HorizontalAlignment::Center,
                ezgui::VerticalAlignment::Center,
            ),
        );
        self.menu.draw(g);
    }
}

fn info_for(id: ID, ui: &UI, ctx: &EventCtx) -> Text {
    let (map, sim, draw_map) = (&ui.primary.map, &ui.primary.sim, &ui.primary.draw_map);
    let mut txt = Text::new();
    // TODO Technically we should recalculate all of this as the window resizes, then.
    txt.override_width = Some(0.7 * ctx.canvas.window_width);
    txt.override_height = Some(0.7 * ctx.canvas.window_height);

    txt.extend(&CommonState::default_osd(id.clone(), ui));

    match id {
        ID::Road(_) => unreachable!(),
        ID::Trip(_) => {}
        ID::Lane(id) => {
            let l = map.get_l(id);
            let r = map.get_r(l.parent);

            txt.add_appended(vec![
                Line(format!("{} is ", l.id)),
                Line(r.get_name()).fg(Color::CYAN),
            ]);
            txt.add(Line(format!(
                "Parent {} (originally {}) points to {}",
                r.id, r.stable_id, r.dst_i
            )));
            txt.add(Line(format!(
                "Lane is {} long, parent {} is {} long",
                l.length(),
                r.id,
                r.center_pts.length()
            )));
            styled_kv(&mut txt, &r.osm_tags);
            if l.is_parking() {
                txt.add(Line(format!(
                    "Has {} parking spots",
                    l.number_parking_spots()
                )));
            } else if l.is_driving() {
                txt.add(Line(format!(
                    "Parking blackhole redirect? {:?}",
                    l.parking_blackhole
                )));
            }
            if let Some(types) = l.get_turn_restrictions(r) {
                txt.add(Line(format!("Turn restriction for this lane: {:?}", types)));
            }
            for (restriction, to) in &r.turn_restrictions {
                txt.add(Line(format!(
                    "Restriction from this road to {}: {:?}",
                    to, restriction
                )));
            }
        }
        ID::Intersection(id) => {
            txt.add(Line(id.to_string()));
            let i = map.get_i(id);
            txt.add(Line(format!("Roads: {:?}", i.roads)));
            txt.add(Line(format!(
                "Orig roads: {:?}",
                i.roads
                    .iter()
                    .map(|r| map.get_r(*r).stable_id)
                    .collect::<Vec<_>>()
            )));
            txt.add(Line(format!("Originally {}", i.stable_id)));
        }
        ID::Turn(id) => {
            let t = map.get_t(id);
            txt.add(Line(format!("{}", id)));
            txt.add(Line(format!("{:?}", t.turn_type)));
        }
        ID::Building(id) => {
            let b = map.get_b(id);
            txt.add(Line(format!("Building #{:?}", id)));
            txt.add(Line(format!(
                "Dist along sidewalk: {}",
                b.front_path.sidewalk.dist_along()
            )));
            styled_kv(&mut txt, &b.osm_tags);
        }
        ID::Car(id) => {
            for line in sim.car_tooltip(id) {
                // TODO Wrap
                txt.add(Line(line));
            }
        }
        ID::Pedestrian(id) => {
            for line in sim.ped_tooltip(id) {
                // TODO Wrap
                txt.add(Line(line));
            }
        }
        ID::PedCrowd(members) => {
            txt.add(Line(format!("Crowd of {}", members.len())));
        }
        ID::ExtraShape(id) => {
            styled_kv(&mut txt, &draw_map.get_es(id).attributes);
        }
        ID::BusStop(id) => {
            txt.add(Line(id.to_string()));
            for r in map.get_all_bus_routes() {
                if r.stops.contains(&id) {
                    txt.add(Line(format!("- Route {}", r.name)));
                }
            }
        }
        ID::Area(id) => {
            let a = map.get_a(id);
            txt.add(Line(format!("{}", id)));
            styled_kv(&mut txt, &a.osm_tags);
        }
    };
    txt
}

fn styled_kv(txt: &mut Text, tags: &BTreeMap<String, String>) {
    for (k, v) in tags {
        txt.add_appended(vec![
            Line(k).fg(Color::RED),
            Line(" = "),
            Line(v).fg(Color::CYAN),
        ]);
    }
}
