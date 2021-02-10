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
    match (name.city.as_ref(), name.map.as_ref()) {
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
        ("allerton_bywater", "center") => "Allerton Bywater",
        ("bellevue", "huge") => "Bellevue",
        ("berlin", "center") => "Berlin (city center)",
        ("cambridge", "great_kneighton") => "Great Kneighton (Cambridge)",
        ("cheshire", "chapelford") => "Chapelford (Cheshire)",
        ("dartford", "ebbsfleet") => "Ebbsfleet (Dartford)",
        ("detroit", "downtown") => "Detroit",
        ("harwell", "didcot") => "Didcot (Harwell)",
        ("krakow", "center") => "Kraków (city center)",
        ("lancaster", "bailrigg") => "Bailrigg (Lancaster)",
        ("leeds", "central") => "Leeds (city center)",
        ("leeds", "huge") => "Leeds (entire area inside motorways)",
        ("leeds", "lcid") => "Leeds Climate Innovation District",
        ("leeds", "north") => "North Leeds",
        ("leeds", "west") => "West Leeds",
        ("london", "southbank") => "London (Southbank)",
        ("montreal", "plateau") => "Montréal (Plateau)",
        ("nyc", "lower_manhattan") => "Lower Manhattan",
        ("nyc", "midtown_manhattan") => "Midtown Manhattan",
        ("paris", "center") => "Paris (city center)",
        ("paris", "north") => "Paris (north)",
        ("paris", "south") => "Paris (south)",
        ("paris", "east") => "Paris (east)",
        ("paris", "west") => "Paris (west)",
        ("providence", "downtown") => "Providence",
        ("poynton", "handforth") => "Handforth (Poynton)",
        ("salzburg", "north") => "Salzburg (north)",
        ("salzburg", "south") => "Salzburg (south)",
        ("salzburg", "east") => "Salzburg (east)",
        ("salzburg", "west") => "Salzburg (west)",
        ("stratford_upon_avon", "long_marston") => "Long Marston (Stratford)",
        ("tel_aviv", "center") => "Tel Aviv (city center)",
        ("warsaw", "center") => "Warsaw (city center)",
        _ => &name.map,
    }
}

pub fn open_browser<I: Into<String>>(url: I) {
    let _ = webbrowser::open(&url.into());
}
