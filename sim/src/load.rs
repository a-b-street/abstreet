use abstutil;
use control::ControlMap;
use map_model::{Edits, Map};
use Sim;

// Convenience method to setup everything.
pub fn load(
    input: String,
    scenario_name: String,
    rng_seed: Option<u8>,
) -> (Map, Edits, ControlMap, Sim) {
    let edits: Edits = abstutil::read_json("road_edits.json").unwrap_or(Edits::new());

    if input.contains("data/save/") {
        println!("Resuming from {}", input);
        let sim: Sim = abstutil::read_json(&input).expect("loading sim state failed");
        // TODO assuming the relative path :(
        let map_path = format!("../data/{}.abst", sim.map_name);
        let map =
            Map::new(&map_path, &edits).expect(&format!("Couldn't load map from {}", map_path));
        let control_map = ControlMap::new(&map);
        (map, edits, control_map, sim)
    } else {
        println!("Loading map {}", input);
        let map = Map::new(&input, &edits).expect("Couldn't load map");
        let control_map = ControlMap::new(&map);
        let sim = Sim::new(&map, scenario_name, rng_seed);
        (map, edits, control_map, sim)
    }
}
