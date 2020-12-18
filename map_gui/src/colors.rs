//! A color scheme groups colors used for different map, dynamic, and UI elements in one place, to
//! encourage deduplication. The player can also switch between different color schemes.

use map_model::osm::RoadRank;
use map_model::LaneType;
use widgetry::{Choice, Color, EventCtx, Fill, Style, Texture};

use crate::tools::{loading_tips, ColorScale};

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

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ColorSchemeChoice {
    Standard,
    NightMode,
    SAMGreenDay,
    SAMDesertDay,
    BAP,
    OSM,
    Starcat,
    Textured,
    MapboxLight,
    MapboxDark,
    FadedZoom,
    NegativeSpace,
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
            Choice::new("textured", ColorSchemeChoice::Textured),
            Choice::new("mapbox light", ColorSchemeChoice::MapboxLight),
            Choice::new("mapbox dark", ColorSchemeChoice::MapboxDark),
            Choice::new("faded zoom", ColorSchemeChoice::FadedZoom),
            Choice::new("negative space", ColorSchemeChoice::NegativeSpace),
        ]
    }
}

pub struct ColorScheme {
    scheme: ColorSchemeChoice,

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
    pub dialog_bg: Color,
    pub minimap_cursor_border: Color,
    pub minimap_cursor_bg: Option<Color>,
    pub minimap_selected_zoom: Color,
    pub minimap_unselected_zoom: Color,

    // Roads
    driving_lane: Color,
    bus_lane: Color,
    parking_lane: Color,
    bike_lane: Color,
    sidewalk: Color,
    pub sidewalk_lines: Option<Color>,
    general_road_marking: Color,
    road_center_line: Color,
    pub light_rail_track: Color,
    pub private_road: Color,
    unzoomed_highway: Color,
    unzoomed_arterial: Color,
    unzoomed_residential: Color,

    // Intersections
    pub normal_intersection: Color,
    pub stop_sign: Color,
    pub stop_sign_pole: Color,
    pub signal_protected_turn: Color,
    pub signal_permitted_turn: Color,
    pub signal_banned_turn: Color,
    pub signal_box: Color,
    pub signal_spinner: Color,
    pub signal_turn_block_bg: Color,

    // Timeline delay highlighting
    pub very_slow_intersection: Color,
    pub slow_intersection: Color,
    pub normal_slow_intersection: Color,

    // Other static elements
    pub void_background: Color,
    pub map_background: Fill,
    pub unzoomed_interesting_intersection: Color,
    pub residential_building: Color,
    pub commercial_building: Color,
    pub building_outline: Color,
    pub parking_lot: Color,
    pub grass: Fill,
    pub water: Fill,
    pub median_strip: Fill,

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
    pub train_body: Color,
    pub ped_head: Color,
    pub ped_foot: Color,
    pub ped_preparing_bike_body: Color,
    pub ped_crowd: Color,
    pub bike_frame: Color,
    pub parked_car: Color,

    // Layers
    pub good_to_bad_red: ColorScale,
    pub good_to_bad_green: ColorScale,
    pub bus_layer: Color,
    pub edits_layer: Color,

    // Misc
    pub parking_trip: Color,
    pub bike_trip: Color,
    pub bus_trip: Color,
    pub before_changes: Color,
    pub after_changes: Color,
}

impl ColorScheme {
    pub fn new(ctx: &mut EventCtx, scheme: ColorSchemeChoice) -> ColorScheme {
        let mut cs = match scheme {
            ColorSchemeChoice::Standard => ColorScheme::standard(),
            ColorSchemeChoice::NightMode => ColorScheme::night_mode(),
            ColorSchemeChoice::SAMGreenDay => ColorScheme::sam_green_day(),
            ColorSchemeChoice::SAMDesertDay => ColorScheme::sam_desert_day(),
            ColorSchemeChoice::BAP => ColorScheme::bap(),
            ColorSchemeChoice::OSM => ColorScheme::osm(),
            ColorSchemeChoice::Starcat => ColorScheme::starcat(),
            ColorSchemeChoice::Textured => ColorScheme::textured(),
            ColorSchemeChoice::MapboxLight => ColorScheme::mapbox_light(),
            ColorSchemeChoice::MapboxDark => ColorScheme::mapbox_dark(),
            ColorSchemeChoice::FadedZoom => ColorScheme::faded_zoom(),
            ColorSchemeChoice::NegativeSpace => ColorScheme::negative_space(),
        };
        cs.scheme = scheme;
        ctx.set_style(cs.gui_style.clone());
        cs
    }

    fn standard() -> ColorScheme {
        let mut gui_style = Style::standard();
        gui_style.loading_tips = loading_tips();
        ColorScheme {
            scheme: ColorSchemeChoice::Standard,

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
            dialog_bg: hex("#94C84A"),
            minimap_cursor_border: Color::BLACK,
            minimap_cursor_bg: None,
            minimap_selected_zoom: Color::WHITE,
            minimap_unselected_zoom: Color::WHITE.alpha(0.2),
            gui_style,

            // Roads
            driving_lane: Color::BLACK,
            bus_lane: Color::rgb(190, 74, 76),
            parking_lane: Color::grey(0.2),
            bike_lane: Color::rgb(15, 125, 75),
            sidewalk: Color::grey(0.8),
            sidewalk_lines: Some(Color::grey(0.7)),
            general_road_marking: Color::WHITE,
            road_center_line: Color::YELLOW,
            light_rail_track: hex("#844204"),
            private_road: hex("#F0B0C0"),
            unzoomed_highway: Color::rgb(232, 146, 162),
            unzoomed_arterial: Color::rgb(255, 199, 62),
            unzoomed_residential: Color::WHITE,

            // Intersections
            normal_intersection: Color::grey(0.2),
            stop_sign: Color::RED,
            stop_sign_pole: Color::grey(0.5),
            signal_protected_turn: hex("#72CE36"),
            signal_permitted_turn: hex("#4CA7E9"),
            signal_banned_turn: Color::BLACK,
            signal_box: Color::grey(0.5),
            signal_spinner: hex("#F2994A"),
            signal_turn_block_bg: Color::grey(0.6),

            // Timeline delay highlighting
            very_slow_intersection: Color::RED,
            slow_intersection: Color::YELLOW,
            normal_slow_intersection: Color::GREEN,

            // Other static elements
            void_background: Color::BLACK,
            map_background: Color::grey(0.87).into(),
            unzoomed_interesting_intersection: Color::BLACK,
            residential_building: hex("#C4C1BC"),
            commercial_building: hex("#9FABA7"),
            building_outline: hex("#938E85"),
            parking_lot: Color::grey(0.7),
            grass: hex("#94C84A").into(),
            water: Color::rgb(164, 200, 234).into(),
            median_strip: Color::CYAN.into(),

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
            train_body: hex("#42B6E9"),
            ped_head: Color::rgb(139, 69, 19),
            ped_foot: Color::BLACK,
            ped_preparing_bike_body: Color::rgb(255, 0, 144),
            ped_crowd: Color::rgb_f(0.2, 0.7, 0.7),
            bike_frame: hex("#AAA9AD"),
            parked_car: hex("#938E85"),

            // Layers
            good_to_bad_red: ColorScale(vec![hex("#F19A93"), hex("#A32015")]),
            good_to_bad_green: ColorScale(vec![hex("#BEDB92"), hex("#397A4C")]),
            bus_layer: hex("#4CA7E9"),
            edits_layer: hex("#12409D"),

            // Misc
            parking_trip: hex("#4E30A6"),
            bike_trip: Color::rgb(15, 125, 75),
            bus_trip: Color::rgb(190, 74, 76),
            before_changes: Color::BLUE,
            after_changes: Color::RED,
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

    pub fn unzoomed_road_surface(&self, rank: RoadRank) -> Color {
        if self.scheme == ColorSchemeChoice::NegativeSpace {
            return Color::BLACK;
        }

        match rank {
            RoadRank::Highway => self.unzoomed_highway,
            RoadRank::Arterial => self.unzoomed_arterial,
            RoadRank::Local => self.unzoomed_residential,
        }
    }

    pub fn zoomed_road_surface(&self, lane: LaneType, rank: RoadRank) -> Color {
        match self.scheme {
            ColorSchemeChoice::FadedZoom => match lane {
                LaneType::Sidewalk | LaneType::Shoulder => match rank {
                    RoadRank::Highway | RoadRank::Arterial => hex("#F2F2F2"),
                    RoadRank::Local => hex("#DBDDE5"),
                },
                _ => match rank {
                    RoadRank::Highway => hex("#F89E59"),
                    RoadRank::Arterial => hex("#F2D163"),
                    RoadRank::Local => hex("#FFFFFF"),
                },
            },
            ColorSchemeChoice::NegativeSpace => Color::BLACK,
            _ => match lane {
                LaneType::Driving => self.driving_lane,
                LaneType::Bus => self.bus_lane,
                LaneType::Parking => self.parking_lane,
                LaneType::Sidewalk | LaneType::Shoulder => self.sidewalk,
                LaneType::Biking => self.bike_lane,
                LaneType::SharedLeftTurn => self.driving_lane,
                LaneType::Construction => self.parking_lane,
                LaneType::LightRail => unreachable!(),
            },
        }
    }
    pub fn zoomed_intersection_surface(&self, rank: RoadRank) -> Color {
        match self.scheme {
            ColorSchemeChoice::FadedZoom => self.zoomed_road_surface(LaneType::Driving, rank),
            _ => self.normal_intersection,
        }
    }

    pub fn road_center_line(&self, rank: RoadRank) -> Color {
        match self.scheme {
            ColorSchemeChoice::FadedZoom => match rank {
                RoadRank::Highway => hex("#60564D"),
                RoadRank::Arterial => hex("#585858"),
                RoadRank::Local => hex("#1C1C1C"),
            },
            _ => self.road_center_line,
        }
    }

    pub fn general_road_marking(&self, rank: RoadRank) -> Color {
        match self.scheme {
            ColorSchemeChoice::FadedZoom => match rank {
                RoadRank::Highway => hex("#60564D"),
                RoadRank::Arterial => hex("#FFFFFF"),
                RoadRank::Local => hex("#BABBBF"),
            },
            _ => self.general_road_marking,
        }
    }

    pub fn solid_road_center(&self) -> bool {
        self.scheme == ColorSchemeChoice::FadedZoom
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
    // Shamelessly adapted from https://github.com/Uriopass/Egregoria
    fn night_mode() -> ColorScheme {
        let mut cs = ColorScheme::standard();
        cs.map_background = Color::BLACK.into();
        cs.grass = Color::hex("#243A1F").into();
        cs.water = Color::hex("#21374E").into();
        cs.residential_building = Color::hex("#5E8962");
        cs.commercial_building = Color::hex("#5D5F97");

        cs.driving_lane = Color::hex("#404040");
        cs.parking_lane = Color::hex("#353535");
        cs.sidewalk = Color::hex("#6B6B6B");
        cs.general_road_marking = Color::hex("#B1B1B1");
        cs.normal_intersection = cs.driving_lane;
        cs.road_center_line = cs.general_road_marking;

        cs.parking_lot = cs.sidewalk;
        cs.unzoomed_arterial = cs.sidewalk;
        cs.unzoomed_highway = cs.parking_lane;
        cs.unzoomed_residential = cs.driving_lane;
        cs.unzoomed_interesting_intersection = cs.unzoomed_highway;
        cs.stop_sign = Color::rgb_f(0.67, 0.55, 0.55);
        cs.private_road = Color::hex("#9E757F");

        cs.panel_bg = Color::hex("#003046").alpha(0.9);
        cs.gui_style.panel_bg = cs.panel_bg;
        cs.inner_panel = cs.panel_bg;
        cs.minimap_cursor_border = Color::WHITE;
        cs.minimap_cursor_bg = Some(Color::rgba(238, 112, 46, 0.2));
        cs.minimap_selected_zoom = Color::hex("#EE702E");
        cs.minimap_unselected_zoom = Color::WHITE.alpha(0.3);

        cs
    }

    fn sam_green_day() -> ColorScheme {
        let mut cs = ColorScheme::standard();
        cs.map_background = hex("#CFE2C4").into();
        cs.water = hex("#B4D3E5").into();
        cs.driving_lane = hex("#C6CDD5");
        cs.residential_building = hex("#CCD4BD");
        cs.sidewalk = hex("#98A1AA");
        cs
    }

    fn sam_desert_day() -> ColorScheme {
        let mut cs = ColorScheme::standard();
        cs.map_background = hex("#FEE4D7").into();
        cs.grass = hex("#F6C6AF").into();
        cs.dialog_bg = hex("#F6C6AF");
        cs.driving_lane = hex("#BECBD3");
        cs.residential_building = hex("#DEAA95");
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
        cs.grass = hex("#84BA3B").into(); // #2F8C2C
        cs.dialog_bg = hex("#84BA3B");
        cs.residential_building = hex("#367335"); // #194C18
        cs.normal_intersection = hex("#4B5485");
        cs.driving_lane = hex("#384173");
        cs.parking_lane = hex("#4B5485");
        cs.sidewalk = hex("#89ABD9");
        cs.sidewalk_lines = Some(hex("#4B5485"));
        cs.general_road_marking = hex("#89ABD9");
        cs.map_background = hex("#589D54").into(); // #153F14
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
        cs.grass = hex("#3F8C0C").into();
        cs.dialog_bg = hex("#3F8C0C");
        cs.residential_building = hex("#8099A8"); // #5E7486
        cs.map_background = hex("#737373").into();
        cs.driving_lane = hex("#2A2A2A"); // TODO for arterial
        cs.road_center_line = hex("#DB952E");
        cs.general_road_marking = hex("#D6D6D6");
        cs.sidewalk = cs.general_road_marking;
        cs.sidewalk_lines = Some(hex("#707070"));
        cs.bike_lane = hex("#72CE36");
        cs.bus_lane = hex("#AD302D");
        cs
    }

    fn textured() -> ColorScheme {
        let mut cs = ColorScheme::standard();
        cs.grass = Texture::GRASS.into();
        cs.water = Texture::STILL_WATER.into();
        cs.map_background = Texture::CONCRETE.into();
        cs
    }

    fn mapbox_light() -> ColorScheme {
        let mut cs = ColorScheme::standard();
        cs.map_background = hex("#F2F3F1").into();
        cs.unzoomed_highway = Color::WHITE;
        cs.unzoomed_arterial = Color::WHITE;
        cs.unzoomed_residential = Color::WHITE;
        cs.grass = hex("#ECEEED").into();
        cs.water = hex("#CAD2D3").into();
        cs.residential_building = hex("#E9E9E7").into();
        cs.commercial_building = hex("#E9E9E7").into();
        cs
    }

    fn mapbox_dark() -> ColorScheme {
        let mut cs = ColorScheme::standard();
        cs.map_background = hex("#343332").into();
        let road = hex("#454545");
        cs.unzoomed_highway = road;
        cs.unzoomed_arterial = road;
        cs.unzoomed_residential = road;
        cs.grass = hex("#323432").into();
        cs.water = hex("#181919").into();
        cs.residential_building = hex("#2C2C2B").into();
        cs.commercial_building = hex("#2C2C2B").into();

        // TODO Things like this could be refactored in zoomed_road_surface
        cs.driving_lane = road;
        cs.parking_lane = road;
        cs.bike_lane = road;
        cs.bus_lane = road;
        cs.sidewalk = Color::grey(0.3);
        cs.sidewalk_lines = Some(road);
        cs.normal_intersection = road;
        cs.general_road_marking = cs.building_outline;
        cs.road_center_line = cs.general_road_marking;
        cs.stop_sign = Color::rgb_f(0.67, 0.55, 0.55);

        cs
    }

    fn faded_zoom() -> ColorScheme {
        let mut cs = ColorScheme::standard();
        cs.unzoomed_highway = hex("#F89E59");
        cs.unzoomed_arterial = hex("#F2D163");
        cs.unzoomed_residential = hex("#FFFFFF");
        cs.sidewalk_lines = None;

        cs.map_background = hex("#E5E4E1").into();
        cs.grass = hex("#B6E59E").into();
        cs.water = hex("#75CFF0").into();

        cs.residential_building = hex("#DCD9D6");
        cs.commercial_building = cs.residential_building;

        cs
    }

    fn negative_space() -> ColorScheme {
        let mut cs = ColorScheme::standard();
        let nonempty_space = Color::BLACK;
        cs.map_background = Color::WHITE.into();
        cs.residential_building = nonempty_space;
        cs.commercial_building = nonempty_space;
        cs.building_outline = nonempty_space;
        cs.normal_intersection = nonempty_space;
        cs.general_road_marking = nonempty_space;
        cs.road_center_line = nonempty_space;
        cs.stop_sign = nonempty_space;
        cs.stop_sign_pole = nonempty_space;
        cs.sidewalk_lines = Some(nonempty_space);
        cs.parking_lot = nonempty_space;
        cs.grass = nonempty_space.into();
        cs.water = nonempty_space.into();
        // TODO Why is this showing up?!
        cs.light_rail_track = Color::INVISIBLE;
        cs
    }
}
