use abstio::MapName;
use abstutil::Timer;
use anyhow::{anyhow, bail, Result};
use map_model::Map;
use prettydiff::text::diff_lines;
use std::path::PathBuf;

/// Run the contents of a .osm through the full map importer with default options.
pub fn import_map(path: String) -> Map {
    let mut timer = Timer::new("convert synthetic map");
    let name = MapName::new("zz", "oneshot", &abstutil::basename(&path));
    let clip = None;
    let raw = convert_osm::convert(
        path,
        name,
        clip,
        convert_osm::Options::default(),
        &mut timer,
    );
    Map::create_from_raw(raw, map_model::RawToMapOptions::default(), &mut timer)
}

/// Obtains a path to a test file (test code only!)
/// This is a convenience function for writing test code. It allow tests code from anywhere in the
/// workspace to access test files (eg input .osm files, golden outputfiles etc) which are stored within
/// the `tests` package.
/// This function make direct reference to the location of this source file (using the `file!()` marco)
/// and hence should only be used in test code and not in any production code.
pub fn get_test_file_path(path: String) -> Result<String> {
    // Get the absolute path to the crate that called was invoked at the cli (or equivalent)
    let maybe_workspace_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let maybe_workspace_dir = std::path::Path::new(&maybe_workspace_dir);
    // Get the relative path to this source file within the workspace
    let this_source_file = String::from(file!());

    // Try a find a suitable way to join the two paths to find something that exists
    let test_file = next_test_file_path(maybe_workspace_dir, &this_source_file);
    if test_file.is_ok() {
        // Now try and match the workspace path with the user requested path
        match next_test_file_path(test_file.as_ref().unwrap(), &path) {
            Ok(pb) => Ok(String::from(pb.to_str().unwrap())),
            Err(e) => Err(e),
        }
    } else {
        panic!("Cannot find the absolute path to {}. Check that this function being called from test code, not production code.", this_source_file);
    }
}

fn next_test_file_path(
    maybe_absolute_dir: &std::path::Path,
    file_path: &String,
) -> Result<PathBuf> {
    let path_to_test = maybe_absolute_dir.join(file_path);
    if path_to_test.exists() {
        Ok(path_to_test)
    } else if maybe_absolute_dir.parent().is_some() {
        next_test_file_path(maybe_absolute_dir.parent().unwrap(), file_path)
    } else {
        Err(anyhow!("Cannot locate file '{}'", file_path))
    }
}

/// Compares a string to the contents of the relevant goldenfile.
/// Pretty prints the differences if necessary
pub fn compare_with_goldenfile(actual: String, goldenfile_path: String) -> Result<()> {
    let goldenfile_path = get_test_file_path(goldenfile_path).unwrap();
    // let expected = String::from_utf8(abstio::slurp_file(&goldenfile_path)?).unwrap().clone().trim();
    let binding = String::from_utf8(abstio::slurp_file(&goldenfile_path)?)
        .unwrap()
        .clone();
    let expected = binding.trim();
    let actual_str = actual.trim();
    if actual_str != expected {
        let lcs = diff_lines(&actual_str, &expected);
        bail!(
            "contents differ from goldenfile {}:\n{}",
            &goldenfile_path,
            lcs
        );
    }
    Ok(())
}
