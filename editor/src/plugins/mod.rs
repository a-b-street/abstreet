pub mod debug;
pub mod edit;
pub mod sim;
pub mod view;

use crate::colors::ColorScheme;
use crate::objects::{DrawCtx, RenderingHints, ID};
use crate::state::{PerMapUI, PluginsPerMap};
use ::sim::{ABTest, OriginDestination, Scenario};
use abstutil;
use abstutil::WeightedUsizeChoice;
use downcast::{
    downcast, downcast_methods, downcast_methods_core, downcast_methods_std, impl_downcast, Any,
};
use ezgui::{Canvas, Color, GfxCtx, Prerender, UserInput, WrappedWizard};
use geom::Duration;
use map_model::{IntersectionID, Map, MapEdits, Neighborhood, NeighborhoodBuilder};

// TODO Split into two types, but then State needs two possible types in its exclusive blocking
// field.
pub trait BlockingPlugin: Any {
    fn color_for(&self, _obj: ID, _ctx: &DrawCtx) -> Option<Color> {
        None
    }

    fn draw(&self, _g: &mut GfxCtx, _ctx: &DrawCtx) {}

    // True if active, false if done
    fn blocking_event(&mut self, _ctx: &mut PluginCtx) -> bool {
        false
    }
    fn blocking_event_with_plugins(
        &mut self,
        ctx: &mut PluginCtx,
        _plugins: &mut PluginsPerMap,
    ) -> bool {
        // By default, redirect to the other one.
        self.blocking_event(ctx)
    }
}

downcast!(BlockingPlugin);

pub trait AmbientPlugin {
    fn ambient_event(&mut self, _ctx: &mut PluginCtx);

    fn color_for(&self, _obj: ID, _ctx: &DrawCtx) -> Option<Color> {
        None
    }
    fn draw(&self, _g: &mut GfxCtx, _ctx: &DrawCtx) {}
}

pub trait AmbientPluginWithPrimaryPlugins {
    fn ambient_event_with_plugins(&mut self, _ctx: &mut PluginCtx, _plugins: &mut PluginsPerMap);
}

pub trait NonblockingPlugin {
    // True means active; false means done, please destroy.
    fn nonblocking_event(&mut self, _ctx: &mut PluginCtx) -> bool;

    fn draw(&self, _g: &mut GfxCtx, _ctx: &DrawCtx) {}
}

// This mirrors many, but not all, of the fields in UI.
pub struct PluginCtx<'a> {
    pub primary: &'a mut PerMapUI,
    pub secondary: &'a mut Option<(PerMapUI, PluginsPerMap)>,
    pub canvas: &'a mut Canvas,
    pub cs: &'a mut ColorScheme,
    pub input: &'a mut UserInput,
    pub hints: &'a mut RenderingHints,
    pub recalculate_current_selection: &'a mut bool,
    // And also a thing from ezgui
    pub prerender: &'a Prerender<'a>,
}

// TODO Further refactoring should be done, but at least group these here to start.
// General principles are to avoid actually deserializing the objects unless needed.

pub fn choose_neighborhood(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    let gps_bounds = map.get_gps_bounds().clone();
    // Load the full object, since various plugins visualize the neighborhood when menuing over it
    wizard
        .choose_something::<Neighborhood>(
            query,
            Box::new(move || Neighborhood::load_all(&map_name, &gps_bounds)),
        )
        .map(|(n, _)| n)
}

pub fn load_neighborhood_builder(
    map: &Map,
    wizard: &mut WrappedWizard,
    query: &str,
) -> Option<NeighborhoodBuilder> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something::<NeighborhoodBuilder>(
            query,
            Box::new(move || abstutil::load_all_objects("neighborhoods", &map_name)),
        )
        .map(|(_, n)| n)
}

pub fn load_scenario(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<Scenario> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something::<Scenario>(
            query,
            Box::new(move || abstutil::load_all_objects("scenarios", &map_name)),
        )
        .map(|(_, s)| s)
}

pub fn choose_scenario(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something::<String>(
            query,
            Box::new(move || abstutil::list_all_objects("scenarios", &map_name)),
        )
        .map(|(n, _)| n)
}

pub fn choose_edits(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something::<String>(
            query,
            Box::new(move || {
                let mut list = abstutil::list_all_objects("edits", &map_name);
                list.push(("no_edits".to_string(), "no_edits".to_string()));
                list
            }),
        )
        .map(|(n, _)| n)
}

pub fn load_edits(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<MapEdits> {
    // TODO Exclude current?
    let map_name = map.get_name().to_string();
    wizard
        .choose_something::<MapEdits>(
            query,
            Box::new(move || {
                let mut list = abstutil::load_all_objects("edits", &map_name);
                list.push(("no_edits".to_string(), MapEdits::new(map_name.clone())));
                list
            }),
        )
        .map(|(_, e)| e)
}

pub fn load_ab_test(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<ABTest> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something::<ABTest>(
            query,
            Box::new(move || abstutil::load_all_objects("ab_tests", &map_name)),
        )
        .map(|(_, t)| t)
}

pub fn input_time(wizard: &mut WrappedWizard, query: &str) -> Option<Duration> {
    wizard.input_something(query, None, Box::new(|line| Duration::parse(&line)))
}

pub fn input_weighted_usize(
    wizard: &mut WrappedWizard,
    query: &str,
) -> Option<WeightedUsizeChoice> {
    wizard.input_something(
        query,
        None,
        Box::new(|line| WeightedUsizeChoice::parse(&line)),
    )
}

// TODO Validate the intersection exists? Let them pick it with the cursor?
pub fn choose_intersection(wizard: &mut WrappedWizard, query: &str) -> Option<IntersectionID> {
    wizard.input_something(
        query,
        None,
        Box::new(|line| usize::from_str_radix(&line, 10).ok().map(IntersectionID)),
    )
}

pub fn choose_origin_destination(
    map: &Map,
    wizard: &mut WrappedWizard,
    query: &str,
) -> Option<OriginDestination> {
    let neighborhood = "Neighborhood";
    let border = "Border intersection";
    if wizard.choose_string(query, vec![neighborhood, border])? == neighborhood {
        choose_neighborhood(map, wizard, query).map(OriginDestination::Neighborhood)
    } else {
        choose_intersection(wizard, query).map(OriginDestination::Border)
    }
}
