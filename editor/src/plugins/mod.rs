use ::sim::{ABTest, OriginDestination, Scenario};
use abstutil;
use abstutil::WeightedUsizeChoice;
use ezgui::WrappedWizard;
use geom::Duration;
use map_model::{IntersectionID, Map, MapEdits, Neighborhood, NeighborhoodBuilder};

// TODO Further refactoring should be done, but at least group these here to start.
// General principles are to avoid actually deserializing the objects unless needed.

pub fn choose_neighborhood(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    let gps_bounds = map.get_gps_bounds().clone();
    // Load the full object, since various plugins visualize the neighborhood when menuing over it
    wizard
        .choose_something_no_keys::<Neighborhood>(
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
        .choose_something_no_keys::<NeighborhoodBuilder>(
            query,
            Box::new(move || abstutil::load_all_objects("neighborhoods", &map_name)),
        )
        .map(|(_, n)| n)
}

pub fn load_scenario(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<Scenario> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something_no_keys::<Scenario>(
            query,
            Box::new(move || abstutil::load_all_objects("scenarios", &map_name)),
        )
        .map(|(_, s)| s)
}

pub fn choose_scenario(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something_no_keys::<String>(
            query,
            Box::new(move || abstutil::list_all_objects("scenarios", &map_name)),
        )
        .map(|(n, _)| n)
}

pub fn choose_edits(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something_no_keys::<String>(
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
        .choose_something_no_keys::<MapEdits>(
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
        .choose_something_no_keys::<ABTest>(
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
