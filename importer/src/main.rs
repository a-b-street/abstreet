fn main() {
    let mut args = abstutil::CmdArgs::new();
    let only_map = args.optional_free();
    args.done();

    if let Some(n) = only_map {
        println!("- Just producing RawMap for {}", n);
        importer::seattle_map(n);
    } else {
        println!("- Producing all RawMaps for Seattle");
        for name in abstutil::list_all_objects("../data/input/polygons".to_string()) {
            importer::seattle_map(name);
        }
    }
}
