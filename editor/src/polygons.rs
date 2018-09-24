use abstutil;
use sim::Neighborhood;
use std;
use std::collections::BTreeMap;

pub fn load_all_polygons(map_name: &str) -> Vec<(String, Neighborhood)> {
    let mut tree: BTreeMap<String, Neighborhood> = BTreeMap::new();
    for entry in std::fs::read_dir(format!("../data/neighborhoods/{}/", map_name)).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        let load: Neighborhood =
            abstutil::read_json(&format!("../data/neighborhoods/{}/{}", map_name, name)).unwrap();
        tree.insert(name, load);
    }
    tree.into_iter().collect()
}
