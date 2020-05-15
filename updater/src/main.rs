use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Error, Write};
use std::process::Command;
use walkdir::WalkDir;

fn main() {
    match std::env::args().skip(1).next() {
        Some(x) => match x.as_ref() {
            "--upload" => {
                upload();
            }
            "--dry" => {
                just_compare();
            }
            x => {
                println!("Unknown argument {}", x);
                std::process::exit(1);
            }
        },
        None => {
            download();
        }
    }
}

fn download() {
    let cities = Cities::load_or_create();
    let local = Manifest::generate();
    let truth = Manifest::load("data/MANIFEST.txt".to_string())
        .unwrap()
        .filter(cities);

    // Anything local need deleting?
    for path in local.0.keys() {
        if !truth.0.contains_key(path) {
            run(Command::new("rm").arg(path));
        }
    }

    // Anything missing or needing updating?
    for (path, entry) in truth.0 {
        if local.0.get(&path).map(|x| &x.checksum) != Some(&entry.checksum) {
            std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
            run(Command::new("curl")
                .arg("--fail")
                .arg("-L")
                .arg("-o")
                .arg("tmp_download.zip")
                .arg(entry.dropbox_url.unwrap()));
            // unzip won't overwrite
            run(Command::new("rm").arg(&path));
            run(Command::new("unzip").arg("tmp_download.zip").arg(path));
            run(Command::new("rm").arg("tmp_download.zip"));
        }
    }
}

fn just_compare() {
    let cities = Cities::load_or_create();
    let local = Manifest::generate();
    let truth = Manifest::load("data/MANIFEST.txt".to_string())
        .unwrap()
        .filter(cities);

    // Anything local need deleting?
    for path in local.0.keys() {
        if !truth.0.contains_key(path) {
            println!("- Remove {}", path);
        }
    }

    // Anything missing or needing updating?
    for (path, entry) in truth.0 {
        if local.0.get(&path).map(|x| &x.checksum) != Some(&entry.checksum) {
            println!("- Update {}", path);
        }
    }
}

fn upload() {
    let remote_base = "/home/dabreegster/Dropbox/abstreet_data";

    let mut local = Manifest::generate();
    let remote = Manifest::load(format!("{}/MANIFEST.txt", remote_base))
        .unwrap_or(Manifest(BTreeMap::new()));

    // Anything remote need deleting?
    for path in remote.0.keys() {
        if !local.0.contains_key(path) {
            run(Command::new("rm").arg(format!("{}/{}.zip", remote_base, path)));
        }
    }

    // Anything missing or needing updating?
    for (path, entry) in &mut local.0 {
        let remote_path = format!("{}/{}.zip", remote_base, path);
        if remote.0.get(path).map(|x| &x.checksum) != Some(&entry.checksum) {
            std::fs::create_dir_all(std::path::Path::new(&remote_path).parent().unwrap()).unwrap();
            run(Command::new("zip").arg(&remote_path).arg(&path));
        }
        // The sharelink shouldn't change
        entry.dropbox_url = remote.0.get(path).map(|x| x.dropbox_url.clone().unwrap());
        if entry.dropbox_url.is_none() {
            let url = run(Command::new("dropbox").arg("sharelink").arg(remote_path))
                .trim()
                .to_string();
            if !url.contains("dropbox.com") {
                panic!("dropbox daemon is sad, slow down");
            }
            entry.dropbox_url = Some(url);
        }
    }

    local.write(format!("{}/MANIFEST.txt", remote_base));
    local.write("data/MANIFEST.txt".to_string());
}

// keyed by path
struct Manifest(BTreeMap<String, Entry>);
struct Entry {
    checksum: String,
    dropbox_url: Option<String>,
}

impl Manifest {
    fn generate() -> Manifest {
        let mut kv = BTreeMap::new();
        for entry in WalkDir::new("data/input")
            .into_iter()
            .chain(WalkDir::new("data/system").into_iter())
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_dir() {
                continue;
            }
            let path = entry.path().display().to_string();
            if path.contains("system/assets/")
                || path.contains("system/fonts")
                || path.contains("system/proposals")
                || path.contains("system/synthetic_maps")
                || path.contains("/polygons/")
            {
                continue;
            }
            let md5sum = if cfg!(target_os = "macos") {
                "md5"
            } else {
                "md5sum"
            };
            let checksum = run(Command::new(md5sum).arg(&path))
                .split(" ")
                .next()
                .unwrap()
                .to_string();
            kv.insert(
                path,
                Entry {
                    checksum,
                    dropbox_url: None,
                },
            );
        }
        Manifest(kv)
    }

    fn write(&self, path: String) {
        let mut f = File::create(&path).unwrap();
        for (path, entry) in &self.0 {
            writeln!(
                f,
                "{},{},{}",
                path,
                entry.checksum,
                entry.dropbox_url.as_ref().unwrap()
            )
            .unwrap();
        }
        println!("- Wrote {}", path);
    }

    fn load(path: String) -> Result<Manifest, Error> {
        let mut kv = BTreeMap::new();
        for line in BufReader::new(File::open(path)?).lines() {
            let line = line?;
            let parts = line.split(",").collect::<Vec<_>>();
            assert_eq!(parts.len(), 3);
            kv.insert(
                parts[0].to_string(),
                Entry {
                    checksum: parts[1].to_string(),
                    dropbox_url: Some(parts[2].to_string()),
                },
            );
        }
        Ok(Manifest(kv))
    }

    fn filter(mut self, cities: Cities) -> Manifest {
        // TODO Temporary hack until directories are organized better
        fn map_belongs_to_city(map: &str, city: &str) -> bool {
            match city {
                "seattle" => {
                    map == "23rd"
                        || map == "ballard"
                        || map == "caphill"
                        || map == "downtown"
                        || map == "intl_district"
                        || map == "lakeslice"
                        || map == "montlake"
                        || map == "mt_baker"
                        || map == "udistrict"
                        || map == "west_seattle"
                }
                "huge_seattle" => map == "huge_seattle",
                "austin" => map == "downtown_atx" || map == "huge_austin",
                _ => panic!("Unknown city {}", city),
            }
        }

        let mut remove = Vec::new();
        for path in self.0.keys() {
            // TODO Some hardcoded weird exceptions
            if !cities.runtime.contains(&"huge_seattle".to_string())
                && path == "data/system/scenarios/montlake/everyone_weekday.bin"
            {
                remove.push(path.clone());
                continue;
            }

            let parts = path.split("/").collect::<Vec<_>>();
            if parts[1] == "input" {
                if parts[2] == "screenshots" {
                    if cities
                        .input
                        .iter()
                        .any(|city| map_belongs_to_city(&parts[3], city))
                    {
                        continue;
                    }
                }
                if parts[2] == "raw_maps" {
                    let map = parts[3].trim_end_matches(".bin");
                    if cities
                        .input
                        .iter()
                        .any(|city| map_belongs_to_city(map, city))
                    {
                        continue;
                    }
                }
                if cities.input.contains(&parts[2].to_string()) {
                    continue;
                }
            } else if parts[1] == "system" {
                if parts[2] == "maps" {
                    let map = parts[3].trim_end_matches(".bin");
                    if cities
                        .runtime
                        .iter()
                        .any(|city| map_belongs_to_city(map, city))
                    {
                        continue;
                    }
                } else {
                    let map = &parts[3];
                    if cities
                        .runtime
                        .iter()
                        .any(|city| map_belongs_to_city(map, city))
                    {
                        continue;
                    }
                }
            } else {
                panic!("Wait what's {}", path);
            }
            remove.push(path.clone());
        }
        for path in remove {
            self.0.remove(&path).unwrap();
        }
        self
    }
}

// What data to download?
struct Cities {
    runtime: Vec<String>,
    input: Vec<String>,
}

impl Cities {
    fn load_or_create() -> Cities {
        let path = "data/config";
        if let Ok(f) = File::open(path) {
            let mut cities = Cities {
                runtime: Vec::new(),
                input: Vec::new(),
            };
            for line in BufReader::new(f).lines() {
                let line = line.unwrap();
                let parts = line.split(": ").collect::<Vec<_>>();
                assert_eq!(parts.len(), 2);
                let list = parts[1]
                    .split(",")
                    .map(|x| x.to_string())
                    .filter(|x| !x.is_empty())
                    .collect::<Vec<_>>();
                if parts[0] == "runtime" {
                    cities.runtime = list;
                } else if parts[0] == "input" {
                    cities.input = list;
                } else {
                    panic!("{} is corrupted, what's {}", path, parts[0]);
                }
            }
            if !cities.runtime.contains(&"seattle".to_string()) {
                panic!(
                    "{}: runtime must contain seattle; the game breaks without this",
                    path
                );
            }
            cities
        } else {
            let mut f = File::create(&path).unwrap();
            writeln!(f, "runtime: seattle").unwrap();
            writeln!(f, "input: ").unwrap();
            println!("- Wrote {}", path);
            Cities {
                runtime: vec!["seattle".to_string()],
                input: vec![],
            }
        }
    }
}

fn run(cmd: &mut Command) -> String {
    println!("> {:?}", cmd);
    String::from_utf8(cmd.output().unwrap().stdout).unwrap()
}
