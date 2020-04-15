use ezgui::{Choice, Color, Style};

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
    SAMGreenDay,
    SAMDesertDay,
    BAP,
    OSM,
    Starcat,
}

impl ColorSchemeChoice {
    pub fn choices() -> Vec<Choice<ColorSchemeChoice>> {
        vec![
            Choice::new("default", ColorSchemeChoice::Standard),
            Choice::new("night mode", ColorSchemeChoice::NightMode),
            Choice::new("sam green day", ColorSchemeChoice::SAMGreenDay),
            Choice::new("sam desert day", ColorSchemeChoice::SAMDesertDay),
            Choice::new("bap", ColorSchemeChoice::BAP),
            Choice::new("osm", ColorSchemeChoice::OSM),
            Choice::new("starcat", ColorSchemeChoice::Starcat),
        ]
    }
}

pub struct ColorScheme {
    // UI
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
    pub fade_map_dark: Color,
    pub gui_style: Style,

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
    agent_colors: Vec<Color>,
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

    // Layers
    pub good_to_bad: [Color; 4],
    pub good_to_bad_monochrome_red: [Color; 4],
    pub good_to_bad_monochrome_green: [Color; 4],
    pub bus_layer: Color,
    pub edits_layer: Color,

    // Misc
    pub parking_trip: Color,
}

impl ColorScheme {
    pub fn new(scheme: ColorSchemeChoice) -> ColorScheme {
        match scheme {
            ColorSchemeChoice::Standard => ColorScheme::standard(),
            ColorSchemeChoice::NightMode => ColorScheme::night_mode(),
            ColorSchemeChoice::SAMGreenDay => ColorScheme::sam_green_day(),
            ColorSchemeChoice::SAMDesertDay => ColorScheme::sam_desert_day(),
            ColorSchemeChoice::BAP => ColorScheme::bap(),
            ColorSchemeChoice::OSM => ColorScheme::osm(),
            ColorSchemeChoice::Starcat => ColorScheme::starcat(),
        }
    }

    fn standard() -> ColorScheme {
        let gui_style = Style::standard();
        ColorScheme {
            // UI
            hovering: gui_style.hovering_color,
            panel_bg: gui_style.panel_bg,
            section_bg: Color::grey(0.5),
            inner_panel: hex("#4C4C4C"),
            day_time_slider: hex("#F4DA22"),
            night_time_slider: hex("#12409D"),
            selected: Color::RED.alpha(0.7),
            current_object: Color::WHITE,
            perma_selected_object: Color::BLUE,
            bottom_bar_id: Color::RED,
            bottom_bar_name: Color::CYAN,
            fade_map_dark: Color::BLACK.alpha(0.6),
            gui_style,

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
            signal_protected_turn: hex("#72CE36"),
            signal_permitted_turn: Color::rgba(76, 167, 233, 0.3),
            signal_permitted_turn_outline: hex("#4CA7E9"),
            signal_banned_turn: Color::BLACK,
            signal_box: Color::grey(0.5),
            signal_spinner: hex("#F2994A"),
            signal_turn_block_bg: Color::grey(0.6),

            // Other static elements
            void_background: Color::BLACK,
            map_background: Color::grey(0.87),
            unzoomed_interesting_intersection: Color::BLACK,
            building: Color::rgb(196, 193, 188),
            grass: hex("#94C84A"),
            water: Color::rgb(164, 200, 234),
            bus_stop: Color::CYAN,
            extra_gis_shape: Color::RED.alpha(0.5),

            // Unzoomed dynamic elements
            unzoomed_car: hex("#A32015"),
            unzoomed_bike: hex("#5D9630"),
            unzoomed_bus: hex("#12409D"),
            unzoomed_pedestrian: hex("#DF8C3D"),

            // Agents
            agent_colors: vec![
                hex("#5C45A0"),
                hex("#3E8BC3"),
                hex("#E1BA13"),
                hex("#96322F"),
                hex("#00A27B"),
            ],
            route: Color::ORANGE.alpha(0.5),
            turn_arrow: hex("#DF8C3D"),
            brake_light: hex("#FF1300"),
            bus_body: Color::rgb(50, 133, 117),
            bus_label: Color::rgb(249, 206, 24),
            ped_head: Color::rgb(139, 69, 19),
            ped_foot: Color::BLACK,
            ped_preparing_bike_body: Color::rgb(255, 0, 144),
            ped_crowd: Color::rgb_f(0.2, 0.7, 0.7),
            bike_frame: Color::rgb(0, 128, 128),

            // Layers
            good_to_bad: [
                hex("#7FFA4D"),
                hex("#F2C94C"),
                hex("#EB5757"),
                hex("#96322F"),
            ],
            good_to_bad_monochrome_red: [
                hex("#F19A93"),
                hex("#E8574B"),
                hex("#C7271A"),
                hex("#A32015"),
            ],
            good_to_bad_monochrome_green: [
                hex("#BEDB92"),
                hex("#77C063"),
                hex("#569358"),
                hex("#397A4C"),
            ],
            bus_layer: hex("#4CA7E9"),
            edits_layer: hex("#12409D"),

            // Misc
            parking_trip: hex("#4E30A6"),
        }
    }

    pub fn rotating_color_plot(&self, idx: usize) -> Color {
        modulo_color(
            &vec![
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
        modulo_color(&self.agent_colors, idx)
    }
}

fn modulo_color(colors: &Vec<Color>, idx: usize) -> Color {
    colors[idx % colors.len()]
}

// Convenience
fn hex(x: &str) -> Color {
    Color::hex(x)
}

// Alternate, in-progress schemes
impl ColorScheme {
    fn night_mode() -> ColorScheme {
        let mut cs = ColorScheme::standard();
        cs.building = hex("#42208B");
        cs.sidewalk = hex("#7C55C8");
        cs.grass = hex("#063D88");
        cs.map_background = hex("#070747");
        cs.unzoomed_arterial = hex("#54247A");
        cs.unzoomed_highway = hex("#DD1F7F");
        cs.unzoomed_residential = hex("#4D51AC");
        cs.water = hex("#2A43AA");
        // Horrible choice, but demonstrate it can be done.
        cs.panel_bg = Color::PURPLE;
        cs.gui_style.panel_bg = Color::PURPLE;
        cs
    }

    fn sam_green_day() -> ColorScheme {
        let mut cs = ColorScheme::standard();
        cs.map_background = hex("#CFE2C4");
        cs.water = hex("#B4D3E5");
        cs.driving_lane = hex("#C6CDD5");
        cs.building = hex("#CCD4BD");
        cs.sidewalk = hex("#98A1AA");
        cs
    }

    fn sam_desert_day() -> ColorScheme {
        let mut cs = ColorScheme::standard();
        cs.map_background = hex("#FEE4D7");
        cs.grass = hex("#F6C6AF");
        cs.driving_lane = hex("#BECBD3");
        cs.building = hex("#DEAA95");
        cs.sidewalk = hex("#8B9EA8");
        cs
    }

    fn bap() -> ColorScheme {
        let mut cs = ColorScheme::standard();
        cs.agent_colors = vec![
            /*hex("#DD5444"),
            hex("#C23E46"),
            hex("#821B38"),
            hex("#BC3101"),*/
            hex("#F44273"),
            hex("#B53A7E"),
            hex("#FF616E"),
            hex("#FA8D37"),
        ];
        cs.grass = hex("#84BA3B"); // #2F8C2C
        cs.building = hex("#367335"); // #194C18
        cs.normal_intersection = hex("#4B5485");
        cs.driving_lane = hex("#384173");
        cs.parking_lane = hex("#4B5485");
        cs.sidewalk = hex("#89ABD9");
        cs.sidewalk_lines = hex("#4B5485");
        cs.general_road_marking = hex("#89ABD9");
        cs.map_background = hex("#589D54"); // #153F14
        cs.ped_crowd = hex("#DD5444");
        cs.road_center_line = hex("#BCFF00");
        cs
    }

    fn osm() -> ColorScheme {
        let mut cs = ColorScheme::standard();
        // TODO normal_intersection, driving_lane, parking_lane depends on osm rank
        cs.general_road_marking = Color::BLACK;
        cs.road_center_line = Color::rgb(202, 177, 39);
        cs
    }

    fn starcat() -> ColorScheme {
        let mut cs = ColorScheme::standard();
        cs.grass = hex("#3F8C0C");
        cs.building = hex("#8099A8"); // #5E7486
        cs.map_background = hex("#737373");
        cs.driving_lane = hex("#2A2A2A"); // TODO for arterial
        cs.road_center_line = hex("#DB952E");
        cs.general_road_marking = hex("#D6D6D6");
        cs.sidewalk = cs.general_road_marking;
        cs.sidewalk_lines = hex("#707070");
        cs.bike_lane = hex("#72CE36");
        cs.bus_lane = hex("#AD302D");
        cs
    }
}

// For now, this won't live in ColorScheme, since the scales are independently chooseable.
#[derive(Clone, Copy, PartialEq)]
pub enum HeatmapColors {
    FullSpectral,
    SingleHue,
}

impl HeatmapColors {
    pub fn choices() -> Vec<Choice<HeatmapColors>> {
        vec![
            Choice::new("full spectral", HeatmapColors::FullSpectral),
            Choice::new("single hue", HeatmapColors::SingleHue),
        ]
    }

    // This is in order from low density to high.
    pub fn colors(self) -> Vec<Color> {
        match self {
            HeatmapColors::FullSpectral => vec![
                hex("#0b2c7a"),
                hex("#1e9094"),
                hex("#0ec441"),
                hex("#7bed00"),
                hex("#f7d707"),
                hex("#e68e1c"),
                hex("#c2523c"),
            ],
            HeatmapColors::SingleHue => vec![
                hex("#FFEBD6"),
                hex("#F5CBAE"),
                hex("#EBA988"),
                hex("#E08465"),
                hex("#D65D45"),
                hex("#CC3527"),
                hex("#C40A0A"),
            ],
        }
    }
}
