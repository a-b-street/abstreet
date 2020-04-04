use abstutil::Timer;
use ezgui::Color;
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

// I've gone back and forth how to organize color scheme code. I was previously against having one
// centralized place with all definitions, because careful naming or comments are needed to explain
// the context of a definition. That's unnecessary when the color is defined in the one place it's
// used. But that was before we started consolidating the color palette in designs, and before we
// started rapidly iterating on totally different schemes.
//
// For the record, the compiler catches typos with this approach, but I don't think I had a single
// bug that took more than 30s to catch and fix in ~1.5 years of the untyped string key. ;)

pub struct ColorScheme {
    old: HashMap<String, Color>,

    // UI. TODO Share with ezgui.
    pub hovering: Color,
    pub panel_bg: Color,
    pub section_bg: Color,
    pub inner_panel: Color,

    // Roads
    pub driving_lane: Color,
    pub bus_lane: Color,
    pub parking_lane: Color,
    pub bike_lane: Color,
    pub under_construction: Color,
    pub sidewalk: Color,
    pub sidewalk_lines: Color,
    pub general_road_marking: Color,
    pub road_center_line: Color,

    // Intersections
    pub border_intersection: Color,
    pub border_arrow: Color,
    pub normal_intersection: Color,
    pub stop_sign: Color,
    pub stop_sign_pole: Color,

    // Unzoomed static elements
    pub map_background: Color,
    pub unzoomed_interesting_intersection: Color,

    // Unzoomed dynamic elements
    pub unzoomed_car: Color,
    pub unzoomed_bike: Color,
    pub unzoomed_bus: Color,
    pub unzoomed_pedestrian: Color,

    // Agent
    pub route: Color,
    pub turn_arrow: Color,
    pub brake_light: Color,
    pub bus_body: Color,
    pub bus_label: Color,
}

// Ideal for editing; values are (hex, alpha value).
#[derive(Serialize, Deserialize)]
struct OverrideColorScheme(BTreeMap<String, (String, f32)>);

impl ColorScheme {
    pub fn default() -> ColorScheme {
        ColorScheme {
            old: HashMap::new(),

            // UI
            hovering: Color::ORANGE,
            panel_bg: Color::grey(0.4),
            section_bg: Color::grey(0.5),
            inner_panel: Color::hex("#4C4C4C"),

            // Roads
            driving_lane: Color::BLACK,
            bus_lane: Color::rgb(190, 74, 76),
            parking_lane: Color::grey(0.2),
            bike_lane: Color::rgb(15, 125, 75),
            under_construction: Color::rgb(255, 109, 0),
            sidewalk: Color::grey(0.8),
            sidewalk_lines: Color::grey(0.7),
            general_road_marking: Color::WHITE,
            road_center_line: Color::YELLOW,

            // Intersections
            border_intersection: Color::rgb(50, 205, 50),
            border_arrow: Color::PURPLE,
            normal_intersection: Color::grey(0.2),
            stop_sign: Color::RED,
            stop_sign_pole: Color::grey(0.5),

            // Unzoomed static elements
            map_background: Color::grey(0.87),
            unzoomed_interesting_intersection: Color::BLACK,

            // Unzoomed dynamic elements
            unzoomed_car: Color::hex("#A32015"),
            unzoomed_bike: Color::hex("#5D9630"),
            unzoomed_bus: Color::hex("#12409D"),
            unzoomed_pedestrian: Color::hex("#DF8C3D"),

            // Agents
            route: Color::ORANGE.alpha(0.5),
            turn_arrow: Color::hex("#DF8C3D"),
            brake_light: Color::hex("#FF1300"),
            bus_body: Color::rgb(50, 133, 117),
            bus_label: Color::rgb(249, 206, 24),
        }
    }

    pub fn load(maybe_path: Option<String>) -> ColorScheme {
        let mut map: HashMap<String, Color> = default_colors();

        // TODO For now, regenerate this manually. If the build system could write in data/system/
        // that'd be great, but...
        if false {
            let mut copy = OverrideColorScheme(BTreeMap::new());
            for (name, c) in &map {
                copy.0.insert(name.clone(), (c.to_hex(), c.a));
            }
            abstutil::write_json("../data/system/override_colors.json".to_string(), &copy);
        }

        if let Some(path) = maybe_path {
            let overrides: OverrideColorScheme = abstutil::read_json(path, &mut Timer::throwaway());
            for (name, (hex, a)) in overrides.0 {
                map.insert(name, Color::hex(&hex).alpha(a));
            }
        }
        let mut cs = ColorScheme::default();
        cs.old = map;
        cs
    }

    // Get, but specify the default inline. The default is extracted before compilation by a script
    // and used to generate default_colors().
    pub fn get_def(&self, name: &str, _default: Color) -> Color {
        self.old[name]
    }

    pub fn get(&self, name: &str) -> Color {
        if let Some(c) = self.old.get(name) {
            *c
        } else {
            panic!("Color {} undefined", name);
        }
    }

    pub fn rotating_color_map(&self, idx: usize) -> Color {
        modulo_color(
            vec![
                Color::RED,
                Color::BLUE,
                Color::GREEN,
                Color::PURPLE,
                Color::BLACK,
            ],
            idx,
        )
    }

    pub fn rotating_color_agents(&self, idx: usize) -> Color {
        modulo_color(
            vec![
                Color::hex("#5C45A0"),
                Color::hex("#3E8BC3"),
                Color::hex("#E1BA13"),
                Color::hex("#96322F"),
                Color::hex("#00A27B"),
            ],
            idx,
        )
    }

    pub fn osm_rank_to_color(&self, rank: usize) -> Color {
        if rank >= 16 {
            // Highway
            Color::rgb(232, 146, 162)
        } else if rank >= 6 {
            // Arterial
            Color::rgb(255, 199, 62)
        } else {
            // Residential
            Color::WHITE
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/init_colors.rs"));

fn modulo_color(colors: Vec<Color>, idx: usize) -> Color {
    colors[idx % colors.len()]
}
