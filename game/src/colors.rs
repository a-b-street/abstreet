use ezgui::Color;

// I've gone back and forth how to organize color scheme code. I was previously against having one
// centralized place with all definitions, because careful naming or comments are needed to explain
// the context of a definition. That's unnecessary when the color is defined in the one place it's
// used. But that was before we started consolidating the color palette in designs, and before we
// started rapidly iterating on totally different schemes.
//
// For the record, the compiler catches typos with this approach, but I don't think I had a single
// bug that took more than 30s to catch and fix in ~1.5 years of the untyped string key. ;)
//
// TODO There are plenty of colors left that aren't captured here. :(

#[derive(Clone, Copy, PartialEq)]
pub enum ColorSchemeChoice {
    Standard,
    NightMode,
}

pub struct ColorScheme {
    // UI. TODO Share with ezgui.
    pub hovering: Color,
    pub panel_bg: Color,
    pub section_bg: Color,
    pub inner_panel: Color,
    pub day_time_slider: Color,
    pub night_time_slider: Color,
    pub selected: Color,
    pub current_object: Color,
    pub perma_selected_object: Color,
    pub bottom_bar_id: Color,
    pub bottom_bar_name: Color,

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
    pub unzoomed_highway: Color,
    pub unzoomed_arterial: Color,
    pub unzoomed_residential: Color,

    // Intersections
    pub border_intersection: Color,
    pub border_arrow: Color,
    pub normal_intersection: Color,
    pub stop_sign: Color,
    pub stop_sign_pole: Color,
    pub signal_protected_turn: Color,
    pub signal_permitted_turn: Color,
    pub signal_permitted_turn_outline: Color,
    pub signal_banned_turn: Color,
    pub signal_box: Color,
    pub signal_spinner: Color,
    pub signal_turn_block_bg: Color,

    // Other static elements
    pub void_background: Color,
    pub map_background: Color,
    pub unzoomed_interesting_intersection: Color,
    pub building: Color,
    pub grass: Color,
    pub water: Color,
    pub bus_stop: Color,
    pub extra_gis_shape: Color,

    // Unzoomed dynamic elements
    pub unzoomed_car: Color,
    pub unzoomed_bike: Color,
    pub unzoomed_bus: Color,
    pub unzoomed_pedestrian: Color,

    // Agents
    pub route: Color,
    pub turn_arrow: Color,
    pub brake_light: Color,
    pub bus_body: Color,
    pub bus_label: Color,
    pub ped_head: Color,
    pub ped_foot: Color,
    pub ped_preparing_bike_body: Color,
    pub ped_crowd: Color,
    pub bike_frame: Color,

    // Misc
    pub associated_object: Color,
    pub parking_trip: Color,
}

impl ColorScheme {
    pub fn new(scheme: ColorSchemeChoice) -> ColorScheme {
        match scheme {
            ColorSchemeChoice::Standard => ColorScheme::standard(),
            ColorSchemeChoice::NightMode => ColorScheme::night_mode(),
        }
    }

    fn standard() -> ColorScheme {
        ColorScheme {
            // UI
            hovering: Color::ORANGE,
            panel_bg: Color::grey(0.4),
            section_bg: Color::grey(0.5),
            inner_panel: Color::hex("#4C4C4C"),
            day_time_slider: Color::hex("#F4DA22"),
            night_time_slider: Color::hex("#12409D"),
            selected: Color::RED.alpha(0.7),
            current_object: Color::WHITE,
            perma_selected_object: Color::BLUE,
            bottom_bar_id: Color::RED,
            bottom_bar_name: Color::CYAN,

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
            unzoomed_highway: Color::rgb(232, 146, 162),
            unzoomed_arterial: Color::rgb(255, 199, 62),
            unzoomed_residential: Color::WHITE,

            // Intersections
            border_intersection: Color::rgb(50, 205, 50),
            border_arrow: Color::PURPLE,
            normal_intersection: Color::grey(0.2),
            stop_sign: Color::RED,
            stop_sign_pole: Color::grey(0.5),
            signal_protected_turn: Color::hex("#72CE36"),
            signal_permitted_turn: Color::rgba(76, 167, 233, 0.3),
            signal_permitted_turn_outline: Color::hex("#4CA7E9"),
            signal_banned_turn: Color::BLACK,
            signal_box: Color::grey(0.5),
            signal_spinner: Color::hex("#F2994A"),
            signal_turn_block_bg: Color::grey(0.6),

            // Other static elements
            void_background: Color::BLACK,
            map_background: Color::grey(0.87),
            unzoomed_interesting_intersection: Color::BLACK,
            building: Color::rgb(196, 193, 188),
            grass: Color::hex("#94C84A"),
            water: Color::rgb(164, 200, 234),
            bus_stop: Color::CYAN,
            extra_gis_shape: Color::RED.alpha(0.5),

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
            ped_head: Color::rgb(139, 69, 19),
            ped_foot: Color::BLACK,
            ped_preparing_bike_body: Color::rgb(255, 0, 144),
            ped_crowd: Color::rgb_f(0.2, 0.7, 0.7),
            bike_frame: Color::rgb(0, 128, 128),

            // Misc
            associated_object: Color::PURPLE,
            parking_trip: Color::hex("#4E30A6"),
        }
    }

    fn night_mode() -> ColorScheme {
        let mut cs = ColorScheme::standard();
        cs.building = Color::hex("#42208B");
        cs.sidewalk = Color::hex("#7C55C8");
        cs.grass = Color::hex("#063D88");
        cs.map_background = Color::hex("#070747");
        cs.unzoomed_arterial = Color::hex("#54247A");
        cs.unzoomed_highway = Color::hex("#DD1F7F");
        cs.unzoomed_residential = Color::hex("#4D51AC");
        cs.water = Color::hex("#2A43AA");
        cs
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
            self.unzoomed_highway
        } else if rank >= 6 {
            self.unzoomed_arterial
        } else {
            self.unzoomed_residential
        }
    }
}

fn modulo_color(colors: Vec<Color>, idx: usize) -> Color {
    colors[idx % colors.len()]
}
