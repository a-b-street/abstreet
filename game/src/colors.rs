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
}

// Ideal for editing; values are (hex, alpha value).
#[derive(Serialize, Deserialize)]
struct OverrideColorScheme(BTreeMap<String, (String, f32)>);

impl ColorScheme {
    pub fn default() -> ColorScheme {
        ColorScheme {
            old: HashMap::new(),

            hovering: Color::ORANGE,
            panel_bg: Color::grey(0.4),
            section_bg: Color::grey(0.5),
            inner_panel: Color::hex("#4C4C4C"),
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
}

include!(concat!(env!("OUT_DIR"), "/init_colors.rs"));

fn modulo_color(colors: Vec<Color>, idx: usize) -> Color {
    colors[idx % colors.len()]
}
