use map_model::osm;
use map_model::raw::RawMap;
use std::collections::BTreeMap;

pub fn find_parking_diffs(map: &RawMap) {
    let mut way_to_tags: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for r in map.roads.values() {
        if r.synthetic() || r.osm_tags.contains_key(osm::INFERRED_PARKING) {
            continue;
        }
        let tags = r.osm_tags.clone();
        if !tags.contains_key(osm::PARKING_LEFT)
            && !tags.contains_key(osm::PARKING_RIGHT)
            && !tags.contains_key(osm::PARKING_BOTH)
        {
            continue;
        }
        way_to_tags.insert(tags[osm::OSM_WAY_ID].clone(), tags);
    }

    for (way, tags) in way_to_tags {
        println!("grab way {}", way);
        for (k, v) in tags {
            println!("  - {} = {}", k, v);
        }

        let url = format!("https://api.openstreetmap.org/api/0.6/way/{}", way);
        println!("Fetching {}", url);
        let resp = reqwest::get(&url).unwrap().text().unwrap();
        let mut tree = xmltree::Element::parse(resp.as_bytes())
            .unwrap()
            .take_child("way")
            .unwrap();
        for elem in &tree.children {
            if elem.name == "tag" {
                println!(
                    "attribs: {} = {}",
                    elem.attributes["k"], elem.attributes["v"]
                );
            }
        }
        tree.attributes.remove("timestamp");
        tree.attributes.remove("changeset");
        tree.attributes.remove("user");
        tree.attributes.remove("uid");
        tree.attributes.remove("visible");
        // TODO bump version

        let mut bytes: Vec<u8> = Vec::new();
        tree.write(&mut bytes).unwrap();
        let out = String::from_utf8(bytes).unwrap();
        let stripped = out.trim_start_matches("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");

        println!("wrote: {}", stripped);
    }
}
