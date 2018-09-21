use abstutil;
use geom::Pt2D;
use std;
use std::collections::BTreeMap;

// Named polygonal regions
#[derive(Serialize, Deserialize, Debug)]
pub struct PolygonSelection {
    pub name: String,
    pub points: Vec<Pt2D>,
}

pub fn load_all_polygons(map_name: &str) -> BTreeMap<String, PolygonSelection> {
    let mut results: BTreeMap<String, PolygonSelection> = BTreeMap::new();
    for entry in std::fs::read_dir(format!("../data/polygons/{}/", map_name)).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        let load: PolygonSelection =
            abstutil::read_json(&format!("../data/polygons/{}/{}", map_name, name)).unwrap();
        results.insert(name, load);
    }
    results
}
