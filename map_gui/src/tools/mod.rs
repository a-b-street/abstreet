//! Assorted tools and UI states that're useful for applications built to display maps.

use std::collections::BTreeSet;

use abstio::MapName;
use geom::Polygon;
use map_model::{IntersectionID, Map, RoadID};
use widgetry::{lctrl, EventCtx, GfxCtx, Key, Line, Text, Widget};

pub use self::camera::{CameraState, DefaultMap};
pub use self::city_picker::CityPicker;
pub use self::colors::{ColorDiscrete, ColorNetwork};
pub use self::draw_overlapping_paths::draw_overlapping_paths;
pub use self::heatmap::{draw_isochrone, make_heatmap, Grid, HeatmapOptions};
pub use self::icons::{goal_marker, start_marker};
pub use self::labels::{DrawRoadLabels, DrawSimpleRoadLabels};
pub use self::minimap::{Minimap, MinimapControls};
pub use self::navigate::Navigator;
pub use self::polygon::EditPolygon;
pub use self::title_screen::{Executable, TitleScreen};
pub use self::trip_files::{TripManagement, TripManagementState};
pub use self::ui::{
    checkbox_per_mode, cmp_count, cmp_dist, cmp_duration, color_for_mode, percentage_bar,
    FilePicker,
};
pub use self::waypoints::{InputWaypoints, WaypointID};
use crate::AppLike;

#[cfg(not(target_arch = "wasm32"))]
pub use self::command::RunCommand;
#[cfg(not(target_arch = "wasm32"))]
pub use self::updater::prompt_to_download_missing_data;

mod camera;
mod city_picker;
mod colors;
#[cfg(not(target_arch = "wasm32"))]
mod command;
pub mod compare_counts;
mod draw_overlapping_paths;
mod heatmap;
mod icons;
#[cfg(not(target_arch = "wasm32"))]
mod importer;
mod labels;
mod minimap;
mod navigate;
mod polygon;
mod title_screen;
mod trip_files;
mod ui;
#[cfg(not(target_arch = "wasm32"))]
mod updater;
mod waypoints;

// Update this ___before___ pushing the commit with "[rebuild] [release]".
const NEXT_RELEASE: &str = "0.3.44";

/// Returns the version of A/B Street to link to. When building for a release, this points to that
/// new release. Otherwise it points to the current dev version.
pub fn version() -> &'static str {
    if cfg!(feature = "release_s3") {
        NEXT_RELEASE
    } else {
        "dev"
    }
}

// TODO This is A/B Street specific
pub fn loading_tips() -> Text {
    Text::from_multiline(vec![
        Line("Have you tried..."),
        Line(""),
        Line("- simulating cities in Britain, Taiwan, Poland, and more?"),
        Line("- the 15-minute neighborhood tool?"),
        Line("- exploring all of the map layers?"),
        Line("- playing 15-minute Santa, our arcade game spin-off?"),
    ])
}

/// Make it clear the map can't be interacted with right now.
pub fn grey_out_map(g: &mut GfxCtx, app: &dyn AppLike) {
    g.fork_screenspace();
    // TODO - OSD height
    g.draw_polygon(
        app.cs().fade_map_dark,
        Polygon::rectangle(g.canvas.window_width, g.canvas.window_height),
    );
    g.unfork();
}

// TODO Associate this with maps, but somehow avoid reading the entire file when listing them.
pub fn nice_map_name(name: &MapName) -> &str {
    match name.city.country.as_ref() {
        "au" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("melbourne", "brunswick") => "Melbourne (Brunswick)",
            ("melbourne", "dandenong") => "Melbourne (Dandenong)",
            _ => &name.map,
        },
        "at" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("salzburg", "north") => "Salzburg (north)",
            ("salzburg", "south") => "Salzburg (south)",
            ("salzburg", "east") => "Salzburg (east)",
            ("salzburg", "west") => "Salzburg (west)",
            _ => &name.map,
        },
        "br" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("sao_paulo", "aricanduva") => "São Paulo (Avenue Aricanduva)",
            ("sao_paulo", "center") => "São Paulo (city center)",
            ("sao_paulo", "sao_miguel_paulista") => "São Miguel Paulista",
            _ => &name.map,
        },
        "ca" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("montreal", "plateau") => "Montréal (Plateau)",
            ("toronto", "dufferin") => "Toronto (Dufferin)",
            ("toronto", "sw") => "Toronto (southwest)",
            _ => &name.map,
        },
        "ch" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("geneva", "center") => "Geneva",
            ("zurich", "center") => "Zürich (city center)",
            ("zurich", "north") => "Zürich (north)",
            ("zurich", "south") => "Zürich (south)",
            ("zurich", "east") => "Zürich (east)",
            ("zurich", "west") => "Zürich (west)",
            _ => &name.map,
        },
        "cl" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("santiago", "bellavista") => "Bellavista (Santiago)",
            _ => &name.map,
        },
        "cz" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("frytek_mistek", "huge") => "Frýdek-Místek (entire area)",
            _ => &name.map,
        },
        "de" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("berlin", "center") => "Berlin (city center)",
            ("berlin", "neukolln") => "Berlin-Neukölln",
            ("bonn", "center") => "Bonn (city center)",
            ("bonn", "nordstadt") => "Bonn (Nordstadt)",
            ("bonn", "venusberg") => "Bonn (Venusberg)",
            ("rostock", "center") => "Rostock",
            _ => &name.map,
        },
        "fr" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("charleville_mezieres", "secteur1") => "Charleville-Mézières (secteur 1)",
            ("charleville_mezieres", "secteur2") => "Charleville-Mézières (secteur 2)",
            ("charleville_mezieres", "secteur3") => "Charleville-Mézières (secteur 3)",
            ("charleville_mezieres", "secteur4") => "Charleville-Mézières (secteur 4)",
            ("charleville_mezieres", "secteur5") => "Charleville-Mézières (secteur 5)",
            ("lyon", "center") => "Lyon",
            ("paris", "center") => "Paris (city center)",
            ("paris", "north") => "Paris (north)",
            ("paris", "south") => "Paris (south)",
            ("paris", "east") => "Paris (east)",
            ("paris", "west") => "Paris (west)",
            _ => &name.map,
        },
        "gb" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("allerton_bywater", "center") => "Allerton Bywater",
            ("ashton_park", "center") => "Ashton Park",
            ("aylesbury", "center") => "Aylesbury",
            ("aylesham", "center") => "Aylesham",
            ("bailrigg", "center") => "Bailrigg (Lancaster)",
            ("bath_riverside", "center") => "Bath Riverside",
            ("bicester", "center") => "Bicester",
            ("bournemouth", "center") => "Bournemouth",
            ("bradford", "center") => "Bradford",
            ("brighton", "center") => "Brighton",
            ("brighton", "shoreham_by_sea") => "Shoreham-by-Sea",
            ("bristol", "east") => "East Bristol",
            ("burnley", "center") => "Burnley",
            ("cambridge", "north") => "North Cambridge",
            ("castlemead", "center") => "Castlemead",
            ("chapelford", "center") => "Chapelford (Cheshire)",
            ("chapeltown_cohousing", "center") => "Chapeltown Cohousing",
            ("chichester", "center") => "Chichester",
            ("chorlton", "center") => "Chorlton",
            ("clackers_brook", "center") => "Clackers Brook",
            ("cricklewood", "center") => "Cricklewood",
            ("culm", "center") => "Culm",
            ("derby", "center") => "Derby",
            ("dickens_heath", "center") => "Dickens Heath",
            ("didcot", "center") => "Didcot (Harwell)",
            ("dunton_hills", "center") => "Dunton Hills",
            ("ebbsfleet", "center") => "Ebbsfleet (Dartford)",
            ("edinburgh", "center") => "Edinburgh",
            ("exeter_red_cow_village", "center") => "Exeter Red Cow Village",
            ("glenrothes", "center") => "Glenrothes (Scotland)",
            ("great_kneighton", "center") => "Great Kneighton (Cambridge)",
            ("halsnhead", "center") => "Halsnead",
            ("hampton", "center") => "Hampton",
            ("inverness", "center") => "Inverness",
            ("kergilliack", "center") => "Kergilliack",
            ("keighley", "center") => "Keighley",
            ("kidbrooke_village", "center") => "Kidbrooke Village",
            ("lcid", "center") => "Leeds Climate Innovation District",
            ("leeds", "central") => "Leeds (city center)",
            ("leeds", "huge") => "Leeds (entire area inside motorways)",
            ("leeds", "north") => "North Leeds",
            ("leeds", "west") => "West Leeds",
            ("lockleaze", "center") => "Lockleaze",
            ("london", "camden") => "Camden",
            ("london", "central") => "Central London",
            ("london", "hackney") => "Hackney",
            ("london", "kennington") => "Kennington (London)",
            ("london", "kingston_upon_thames") => "Kingston upon Thames",
            ("london", "southwark") => "Southwark",
            ("long_marston", "center") => "Long Marston (Stratford)",
            ("manchester", "levenshulme") => "Levenshulme (Manchester)",
            ("manchester", "stockport") => "Stockport",
            ("marsh_barton", "center") => "Marsh Barton",
            ("micklefield", "center") => "Micklefield",
            ("newborough_road", "center") => "Newborough Road",
            ("newcastle_great_park", "center") => "Newcastle Great Park",
            ("newcastle_upon_tyne", "center") => "Newcastle upon Tyne",
            ("nottingham", "center") => "Nottingham (city center)",
            ("nottingham", "huge") => "Nottingham (entire area)",
            ("nottingham", "stapleford") => "Stapleford",
            ("northwick_park", "center") => "Northwick Park",
            ("oxford", "center") => "Oxford",
            ("poundbury", "center") => "Poundbury",
            ("priors_hall", "center") => "Priors Hall",
            ("sheffield", "darnall") => "Darnall",
            ("st_albans", "center") => "St Albans",
            ("taunton_firepool", "center") => "Taunton Firepool",
            ("taunton_garden", "center") => "Taunton Garden",
            ("tresham", "center") => "Tresham",
            ("trumpington_meadows", "center") => "Trumpington Meadows",
            ("tyersal_lane", "center") => "Tyersal Lane",
            ("upton", "center") => "Upton",
            ("water_lane", "center") => "Water Lane",
            ("wichelstowe", "center") => "Wichelstowe",
            ("wixams", "center") => "Wixams",
            ("wokingham", "center") => "Wokingham",
            ("wynyard", "center") => "Wynyard",
            _ => &name.map,
        },
        "il" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("tel_aviv", "center") => "Tel Aviv (city center)",
            _ => &name.map,
        },
        "in" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("pune", "center") => "Pune",
            _ => &name.map,
        },
        "ir" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("tehran", "parliament") => "Tehran (near Parliament)",
            _ => &name.map,
        },
        "jp" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("hiroshima", "uni") => "Hiroshima University",
            _ => &name.map,
        },
        "ly" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("tripoli", "center") => "Tripoli",
            _ => &name.map,
        },
        "nl" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("groningen", "center") => "Groningen (city center)",
            ("groningen", "huge") => "Groningen (entire area)",
            _ => &name.map,
        },
        "nz" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("auckland", "mangere") => "Māngere (Auckland)",
            _ => &name.map,
        },
        "pl" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("krakow", "center") => "Kraków (city center)",
            ("warsaw", "center") => "Warsaw (city center)",
            _ => &name.map,
        },
        "pt" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("lisbon", "center") => "Lisbon (city center)",
            _ => &name.map,
        },
        "sg" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("jurong", "center") => "Jurong",
            _ => &name.map,
        },
        "tw" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("keelung", "center") => "Keelung",
            ("taipei", "center") => "Taipei (city center)",
            _ => &name.map,
        },
        "us" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("anchorage", "downtown") => "Anchorage",
            ("bellevue", "huge") => "Bellevue",
            ("beltsville", "i495") => "I-495 in Beltsville, MD",
            ("detroit", "downtown") => "Detroit",
            ("lynnwood", "hazelwood") => "Lynnwood, WA",
            ("milwaukee", "downtown") => "Downtown Milwaukee",
            ("milwaukee", "oak_creek") => "Oak Creek",
            ("mt_vernon", "burlington") => "Burlington",
            ("mt_vernon", "downtown") => "Mt. Vernon",
            ("nyc", "fordham") => "Fordham",
            ("nyc", "lower_manhattan") => "Lower Manhattan",
            ("nyc", "midtown_manhattan") => "Midtown Manhattan",
            ("nyc", "downtown_brooklyn") => "Downtown Brooklyn",
            ("phoenix", "gilbert") => "Gilbert",
            ("phoenix", "tempe") => "Tempe",
            ("providence", "downtown") => "Providence",
            ("san_francisco", "downtown") => "San Francisco",
            ("seattle", "arboretum") => "Arboretum",
            ("seattle", "central_seattle") => "Central Seattle",
            ("seattle", "downtown") => "Downtown Seattle",
            ("seattle", "huge_seattle") => "Seattle (entire area)",
            ("seattle", "lakeslice") => "Lake Washington corridor",
            ("seattle", "montlake") => "Montlake and Eastlake",
            ("seattle", "north_seattle") => "North Seattle",
            ("seattle", "phinney") => "Phinney Ridge",
            ("seattle", "qa") => "Queen Anne",
            ("seattle", "slu") => "South Lake Union",
            ("seattle", "south_seattle") => "South Seattle",
            ("seattle", "udistrict_ravenna") => "University District",
            ("seattle", "wallingford") => "Wallingford",
            ("seattle", "west_seattle") => "West Seattle",
            ("tucson", "center") => "Tucson",
            _ => &name.map,
        },
        _ => &name.map,
    }
}

pub fn nice_country_name(code: &str) -> &str {
    // If you add something here, please also add the flag to data/system/assets/flags.
    // https://github.com/hampusborgos/country-flags/tree/main/svg
    match code {
        "au" => "Australia",
        "at" => "Austria",
        "br" => "Brazil",
        "ca" => "Canada",
        "ch" => "Switzerland",
        "cl" => "Chile",
        "cz" => "Czech Republic",
        "de" => "Germany",
        "fr" => "France",
        "gb" => "Great Britain",
        "il" => "Israel",
        "in" => "India",
        "ir" => "Iran",
        "jp" => "Japan",
        "ly" => "Libya",
        "nl" => "Netherlands",
        "nz" => "New Zealand",
        "pl" => "Poland",
        "pt" => "Portugal",
        "sg" => "Singapore",
        "tw" => "Taiwan",
        "us" => "United States of America",
        _ => code,
    }
}

/// Returns the path to an executable. Native-only.
pub fn find_exe(cmd: &str) -> String {
    let mut directories = Vec::new();
    // Some cargo configurations explicitly use a platform-specific directory
    for arch in ["x86_64-unknown-linux-gnu", ""] {
        // When running from source, prefer release builds, but fallback to debug. This might be
        // confusing when developing and not recompiling in release mode.
        for mode in ["release", "debug"] {
            for relative_dir in [".", "..", "../.."] {
                directories.push(
                    std::path::Path::new(relative_dir)
                        .join("target")
                        .join(arch)
                        .join(mode)
                        .display()
                        .to_string(),
                );
            }
        }
    }
    // When running from the .zip release
    directories.push(".".to_string());

    for dir in directories {
        // Apparently std::path on Windows doesn't do any of this correction. We could build up a
        // PathBuf properly, I guess
        let path = if cfg!(windows) {
            format!("{}/{}.exe", dir, cmd).replace("/", "\\")
        } else {
            format!("{}/{}", dir, cmd)
        };
        if let Ok(metadata) = fs_err::metadata(&path) {
            if metadata.is_file() {
                return path;
            } else {
                debug!(
                    "found matching path: {}/{} but it's not a file.",
                    &path, cmd
                );
            }
        }
    }
    panic!("Couldn't find the {} executable. Is it built?", cmd);
}

/// A button to change maps, with default keybindings
pub fn change_map_btn(ctx: &EventCtx, app: &dyn AppLike) -> Widget {
    ctx.style()
        .btn_popup_icon_text(
            "system/assets/tools/map.svg",
            nice_map_name(app.map().get_name()),
        )
        .hotkey(lctrl(Key::L))
        .build_widget(ctx, "change map")
}

/// A button to return to the title screen
pub fn home_btn(ctx: &EventCtx) -> Widget {
    ctx.style()
        .btn_plain
        .btn()
        .image_path("system/assets/pregame/logo.svg")
        .image_dims(50.0)
        .build_widget(ctx, "Home")
}

/// A standard way to group a home button back to the title screen, the title of the current app,
/// and a button to change maps. Callers must handle the `change map` and `home` click events.
pub fn app_header(ctx: &EventCtx, app: &dyn AppLike, title: &str) -> Widget {
    Widget::col(vec![
        Widget::row(vec![
            home_btn(ctx),
            Line(title).small_heading().into_widget(ctx).centered_vert(),
        ]),
        change_map_btn(ctx, app),
    ])
}

pub fn intersections_from_roads(roads: &BTreeSet<RoadID>, map: &Map) -> BTreeSet<IntersectionID> {
    let mut results = BTreeSet::new();
    for r in roads {
        let r = map.get_r(*r);
        for i in [r.src_i, r.dst_i] {
            if results.contains(&i) {
                continue;
            }
            if map.get_i(i).roads.iter().all(|r| roads.contains(r)) {
                results.insert(i);
            }
        }
    }
    results
}

/// Modify the current URL to set the first free parameter to the current map name.
pub fn update_url_map_name(app: &dyn AppLike) {
    widgetry::tools::URLManager::update_url_free_param(
        app.map()
            .get_name()
            .path()
            .strip_prefix(&abstio::path(""))
            .unwrap()
            .to_string(),
    );
}
