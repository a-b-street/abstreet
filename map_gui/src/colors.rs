//! A color scheme groups colors used for different map, dynamic, and UI elements in one place, to
//! encourage deduplication. The player can also switch between different color schemes.

use std::fs::File;
use std::io::Write;

use anyhow::Result;
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum ColorSchemeChoice {
    DayMode,
    NightMode,
    Pregame,
    Textured,
    FadedZoom,
}

impl ColorSchemeChoice {
    pub fn choices() -> Vec<Choice<ColorSchemeChoice>> {
        vec![
            Choice::new("day mode", ColorSchemeChoice::DayMode),
            Choice::new("night mode", ColorSchemeChoice::NightMode),
            Choice::new("pregame", ColorSchemeChoice::Pregame),
            Choice::new("textured", ColorSchemeChoice::Textured),
            Choice::new("faded zoom", ColorSchemeChoice::FadedZoom),
        ]
    }
}

pub struct ColorScheme {
    scheme: ColorSchemeChoice,

    // UI
    pub panel_bg: Color,
    pub inner_panel_bg: Color,
    pub day_time_slider: Color,
    pub night_time_slider: Color,
    pub selected: Color,
    pub current_object: Color,
    pub perma_selected_object: Color,
    pub fade_map_dark: Color,
    gui_style: Style,
    pub dialog_bg: Color,
    pub minimap_cursor_border: Color,
    pub minimap_cursor_bg: Option<Color>,

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
    pub unzoomed_trail: Color,

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

    // Problems encountered on a trip
    pub slowest_intersection: Color,
    pub slower_intersection: Color,
    pub slow_intersection: Color,

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
    pub pedestrian_plaza: Fill,
    pub study_area: Fill,

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
            ColorSchemeChoice::DayMode => ColorScheme::day_mode(),
            ColorSchemeChoice::NightMode => ColorScheme::night_mode(),
            ColorSchemeChoice::Pregame => ColorScheme::pregame(),
            ColorSchemeChoice::Textured => ColorScheme::textured(),
            ColorSchemeChoice::FadedZoom => ColorScheme::faded_zoom(),
        };
        cs.scheme = scheme;
        ctx.set_style(cs.gui_style.clone());
        cs
    }

    fn pregame() -> ColorScheme {
        let mut cs = Self::light_background(Style::pregame());
        cs.scheme = ColorSchemeChoice::Pregame;
        cs
    }

    fn day_mode() -> ColorScheme {
        let mut cs = Self::light_background(Style::light_bg());
        cs.scheme = ColorSchemeChoice::DayMode;
        cs
    }

    fn light_background(mut gui_style: Style) -> ColorScheme {
        gui_style.loading_tips = loading_tips();
        ColorScheme {
            scheme: ColorSchemeChoice::DayMode,

            // UI
            panel_bg: gui_style.panel_bg,
            inner_panel_bg: gui_style.section_bg,
            day_time_slider: hex("#F4DA22"),
            night_time_slider: hex("#12409D"),
            selected: Color::RED.alpha(0.7),
            current_object: Color::WHITE,
            perma_selected_object: Color::BLUE,
            fade_map_dark: Color::BLACK.alpha(0.6),
            dialog_bg: hex("#94C84A"),
            minimap_cursor_border: Color::BLACK,
            minimap_cursor_bg: None,
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
            unzoomed_highway: hex("#E892A2"),
            unzoomed_arterial: hex("#FFC73E"),
            unzoomed_residential: Color::WHITE,
            unzoomed_trail: hex("#0F7D4B"),

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

            // Problems encountered on a trip
            slowest_intersection: Color::RED,
            slower_intersection: Color::YELLOW,
            slow_intersection: Color::GREEN,

            // Other static elements
            void_background: Color::BLACK,
            map_background: Color::grey(0.87).into(),
            unzoomed_interesting_intersection: Color::BLACK,
            residential_building: hex("#C4C1BC"),
            commercial_building: hex("#9FABA7"),
            building_outline: hex("#938E85"),
            parking_lot: Color::grey(0.7),
            grass: hex("#94C84A").into(),
            water: hex("#A4C8EA").into(),
            median_strip: Color::CYAN.into(),
            pedestrian_plaza: hex("#DDDDE8").into(),
            study_area: hex("#96830C").into(),

            // Unzoomed dynamic elements
            unzoomed_car: hex("#FE5f55"),
            unzoomed_bike: hex("#90BE6D"),
            unzoomed_bus: hex("#FFD166"),
            unzoomed_pedestrian: hex("#457B9D"),

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
            &[
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
            _ => match lane {
                LaneType::Driving => self.driving_lane,
                LaneType::Bus => self.bus_lane,
                LaneType::Parking => self.parking_lane,
                LaneType::Sidewalk | LaneType::Shoulder => self.sidewalk,
                LaneType::Biking => self.bike_lane,
                LaneType::SharedLeftTurn => self.driving_lane,
                LaneType::Construction => self.parking_lane,
                LaneType::LightRail => unreachable!(),
                LaneType::Buffer(_) => self.driving_lane,
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

    // These two could try to use serde, but... Color serializes with a separate RGB by default,
    // and changing it to use a nice hex string is way too hard.
    pub fn export(&self, path: &str) -> Result<()> {
        let mut f = File::create(path)?;
        writeln!(f, "unzoomed_highway {}", self.unzoomed_highway.as_hex())?;
        writeln!(f, "unzoomed_arterial {}", self.unzoomed_arterial.as_hex())?;
        writeln!(
            f,
            "unzoomed_residential {}",
            self.unzoomed_residential.as_hex()
        )?;
        writeln!(f, "unzoomed_trail {}", self.unzoomed_trail.as_hex())?;
        writeln!(f, "private_road {}", self.private_road.as_hex())?;
        writeln!(
            f,
            "residential_building {}",
            self.residential_building.as_hex()
        )?;
        writeln!(
            f,
            "commercial_building {}",
            self.commercial_building.as_hex()
        )?;
        if let Fill::Color(c) = self.grass {
            writeln!(f, "grass {}", c.as_hex())?;
        }
        if let Fill::Color(c) = self.water {
            writeln!(f, "water {}", c.as_hex())?;
        }
        Ok(())
    }

    pub fn import(&mut self, path: &str) -> Result<()> {
        let raw = String::from_utf8(abstio::slurp_file(path)?)?;
        let mut colors = Vec::new();
        for line in raw.split('\n') {
            if line.is_empty() {
                continue;
            }
            let mut parts = line.split(' ');
            parts.next();
            colors.push(Color::hex(parts.next().unwrap()));
        }

        self.unzoomed_highway = colors[0];
        self.unzoomed_arterial = colors[1];
        self.unzoomed_residential = colors[2];
        self.unzoomed_trail = colors[3];
        self.private_road = colors[4];
        self.residential_building = colors[5];
        self.commercial_building = colors[6];
        self.grass = Fill::Color(colors[7]);
        self.water = Fill::Color(colors[8]);

        Ok(())
    }
}

fn modulo_color(colors: &[Color], idx: usize) -> Color {
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
        let mut cs = ColorScheme::day_mode();
        cs.gui_style = widgetry::Style::dark_bg();

        cs.void_background = hex("#200A24");
        cs.map_background = Color::BLACK.into();
        cs.grass = hex("#243A1F").into();
        cs.water = hex("#21374E").into();
        cs.residential_building = hex("#2C422E");
        cs.commercial_building = hex("#5D5F97");

        cs.driving_lane = hex("#404040");
        cs.parking_lane = hex("#353535");
        cs.sidewalk = hex("#6B6B6B");
        cs.general_road_marking = hex("#B1B1B1");
        cs.normal_intersection = cs.driving_lane;
        cs.road_center_line = cs.general_road_marking;

        cs.parking_lot = cs.sidewalk;
        cs.unzoomed_highway = cs.parking_lane;
        cs.unzoomed_arterial = cs.sidewalk;
        cs.unzoomed_residential = cs.driving_lane;
        cs.unzoomed_interesting_intersection = cs.unzoomed_highway;
        cs.stop_sign = hex("#A32015");
        cs.private_road = hex("#9E757F");
        cs.pedestrian_plaza = hex("#94949C").into();
        cs.study_area = hex("#D9B002").into();

        cs.panel_bg = cs.gui_style.panel_bg;
        cs.inner_panel_bg = cs.panel_bg.alpha(1.0);
        cs.minimap_cursor_border = Color::WHITE;
        cs.minimap_cursor_bg = Some(Color::rgba(238, 112, 46, 0.2));

        cs
    }

    fn textured() -> ColorScheme {
        let mut cs = ColorScheme::day_mode();
        cs.grass = Texture::GRASS.into();
        cs.water = Texture::STILL_WATER.into();
        cs.map_background = Texture::CONCRETE.into();
        cs
    }

    fn faded_zoom() -> ColorScheme {
        let mut cs = ColorScheme::day_mode();
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
}
