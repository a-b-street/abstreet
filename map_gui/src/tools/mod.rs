//! Assorted tools and UI states that're useful for applications built to display maps.

use abstio::MapName;
use geom::Polygon;
use widgetry::{GfxCtx, Line, Text};

pub use self::camera::CameraState;
pub use self::city_picker::CityPicker;
pub use self::colors::{ColorDiscrete, ColorLegend, ColorNetwork, ColorScale, DivergingScale};
pub use self::heatmap::{make_heatmap, Grid, HeatmapOptions};
pub use self::minimap::{Minimap, MinimapControls};
pub use self::navigate::Navigator;
pub use self::turn_explorer::TurnExplorer;
pub use self::ui::{ChooseSomething, PopupMsg, PromptInput};
use crate::AppLike;

mod camera;
mod city_picker;
mod colors;
mod heatmap;
mod minimap;
mod navigate;
mod turn_explorer;
mod ui;
#[cfg(not(target_arch = "wasm32"))]
mod updater;

// TODO This is A/B Street specific
pub fn loading_tips() -> Text {
    Text::from_multiline(vec![
        Line("Recent changes (November 8)"),
        Line(""),
        Line("- Download more cities from within the game"),
        Line("- You can now click agents while zoomed out"),
        Line("- New OpenStreetMap viewer, open it from the splash screen"),
        Line("- A web version has launched!"),
        Line("- Slow segments of a trip shown in the info panel"),
        Line("- Alleyways are now included in the map"),
        Line("- Check out the trip tables and summary changes (press 'q')"),
        Line("- Try out the new traffic signal editor!"),
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
        "at" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("salzburg", "north") => "Salzburg (north)",
            ("salzburg", "south") => "Salzburg (south)",
            ("salzburg", "east") => "Salzburg (east)",
            ("salzburg", "west") => "Salzburg (west)",
            _ => &name.map,
        },
        "ca" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("montreal", "plateau") => "Montréal (Plateau)",
            _ => &name.map,
        },
        "de" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("berlin", "center") => "Berlin (city center)",
            ("rostock", "center") => "Rostock",
            _ => &name.map,
        },
        "fr" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("charleville_mezieres", "secteur1") => "Charleville-Mézières (secteur 1)",
            ("charleville_mezieres", "secteur2") => "Charleville-Mézières (secteur 2)",
            ("charleville_mezieres", "secteur3") => "Charleville-Mézières (secteur 3)",
            ("charleville_mezieres", "secteur4") => "Charleville-Mézières (secteur 4)",
            ("charleville_mezieres", "secteur5") => "Charleville-Mézières (secteur 5)",
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
            ("castlemead", "center") => "Castlemead",
            ("chapelford", "center") => "Chapelford (Cheshire)",
            ("clackers_brook", "center") => "Clackers Brook",
            ("culm", "center") => "Culm",
            ("dickens_heath", "center") => "Dickens Heath",
            ("didcot", "center") => "Didcot (Harwell)",
            ("dunton_hills", "center") => "Dunton Hills",
            ("ebbsfleet", "center") => "Ebbsfleet (Dartford)",
            ("great_kneighton", "center") => "Great Kneighton (Cambridge)",
            ("hampton", "center") => "Hampton",
            ("kidbrooke_village", "center") => "Kidbrooke Village",
            ("leeds", "central") => "Leeds (city center)",
            ("leeds", "huge") => "Leeds (entire area inside motorways)",
            ("leeds", "north") => "North Leeds",
            ("leeds", "west") => "West Leeds",
            ("london", "southbank") => "London (Southbank)",
            ("long_marston", "center") => "Long Marston (Stratford)",
            ("micklefield", "center") => "Micklefield",
            ("newcastle_great_park", "center") => "Newcastle Great Park",
            ("poundbury", "center") => "Poundbury",
            ("priors_hall", "center") => "Priors Hall",
            ("taunton_firepool", "center") => "Taunton Firepool",
            ("taunton_garden", "center") => "Taunton Garden",
            ("tresham", "center") => "Tresham",
            ("trumpington_meadows", "center") => "Trumpington Meadows",
            ("tyersal_lane", "center") => "Tyersal Lane",
            ("upton", "center") => "Upton",
            ("wichelstowe", "center") => "Wichelstowe",
            ("wixams", "center") => "Wixams",
            _ => &name.map,
        },
        "il" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("tel_aviv", "center") => "Tel Aviv (city center)",
            _ => &name.map,
        },
        "pl" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("krakow", "center") => "Kraków (city center)",
            ("warsaw", "center") => "Warsaw (city center)",
            _ => &name.map,
        },
        "us" => match (name.city.city.as_ref(), name.map.as_ref()) {
            ("anchorage", "downtown") => "Anchorage",
            ("bellevue", "huge") => "Bellevue",
            ("detroit", "downtown") => "Detroit",
            ("nyc", "lower_manhattan") => "Lower Manhattan",
            ("nyc", "midtown_manhattan") => "Midtown Manhattan",
            ("providence", "downtown") => "Providence",
            ("seattle", "ballard") => "Ballard",
            ("seattle", "downtown") => "Downtown Seattle",
            ("seattle", "huge_seattle") => "Seattle (entire area)",
            ("seattle", "lakeslice") => "Lake Washington corridor",
            ("seattle", "montlake") => "Montlake and Eastlake",
            ("seattle", "north_seattle") => "North Seattle",
            ("seattle", "phinney") => "Phinney Ridge",
            ("seattle", "qa") => "Queen Anne",
            ("seattle", "rainier_valley") => "Rainier Valley",
            ("seattle", "slu") => "South Lake Union",
            ("seattle", "south_seattle") => "South Seattle",
            ("seattle", "udistrict") => "University District",
            ("seattle", "udistrict_ravenna") => "University District / Ravenna",
            ("seattle", "wallingford") => "Wallingford",
            ("seattle", "west_seattle") => "West Seattle",
            _ => &name.map,
        },
        _ => &name.map,
    }
}

pub fn nice_country_name(code: &str) -> &str {
    // If you add something here, please also add the flag to data/system/assets/flags.
    // https://github.com/hampusborgos/country-flags/tree/master/svg
    match code {
        "at" => "Austria",
        "ca" => "Canada",
        "de" => "Germany",
        "fr" => "France",
        "gb" => "Great Britain",
        "il" => "Israel",
        "pl" => "Poland",
        "us" => "United States of America",
        _ => code,
    }
}

pub fn open_browser<I: Into<String>>(url: I) {
    let _ = webbrowser::open(&url.into());
}
