use std::collections::BTreeMap;
use std::error::Error;
use std::fs::{create_dir_all, remove_file, set_permissions, File, Permissions};
use std::io::{copy, BufRead, BufReader, Read, Write};
use std::process::Command;
use walkdir::WalkDir;

const MD5_BUF_READ_SIZE: usize = 4096;
const TMP_DOWNLOAD_NAME: &str = "tmp_download.zip";

#[tokio::main]
async fn main() {
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
            download().await;
        }
    }
}

async fn download() {
    let cities = Cities::load_or_create();
    let local = Manifest::generate();
    let truth = Manifest::load("data/MANIFEST.txt".to_string())
        .unwrap()
        .filter(cities);

    // Anything local need deleting?
    for path in local.0.keys() {
        if !truth.0.contains_key(path) {
            rm(&path);
        }
    }

    // Anything missing or needing updating?
    for (path, entry) in truth.0 {
        if local.0.get(&path).map(|x| &x.checksum) != Some(&entry.checksum) {
            std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
            match curl(entry).await {
                Ok(()) => {
                    unzip(&path);
                }
                Err(e) => {
                    println!("{}, continuing", e);
                }
            };
            // Whether or not download failed, still try to clean up tmp file
            rm(TMP_DOWNLOAD_NAME);
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
            rm(&format!("{}/{}.zip", remote_base, path));
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
            // Dropbox crashes when trying to upload lots of tiny screenshots. :D
            std::thread::sleep(std::time::Duration::from_millis(1000));
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

            println!("> compute md5sum of {}", path);

            // since these files can be very large, computes the md5 hash in chunks
            let mut file = File::open(&path).unwrap();
            let mut buffer = [0 as u8; MD5_BUF_READ_SIZE];
            let mut context = md5::Context::new();
            while let Ok(n) = file.read(&mut buffer) {
                if n == 0 {
                    break;
                }
                context.consume(&buffer[..n]);
            }
            let checksum = format!("{:x}", context.compute());
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

    fn load(path: String) -> Result<Manifest, Box<dyn Error>> {
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
                    map == "ballard"
                        || map == "downtown"
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
                } else if parts[2] == "cities" {
                    if cities.runtime.contains(&basename(parts[3])) {
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

fn basename(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .unwrap()
        .to_os_string()
        .into_string()
        .unwrap()
}

fn run(cmd: &mut Command) -> String {
    println!("> {:?}", cmd);
    String::from_utf8(cmd.output().unwrap().stdout).unwrap()
}

fn rm(path: &str) {
    println!("> rm {}", path);
    match remove_file(path) {
        Ok(_) => {}
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => {
                println!("file {} does not exist, continuing", &path);
            }
            other_error => {
                panic!("problem removing file: {:?}", other_error);
            }
        },
    }
}

async fn curl(entry: Entry) -> Result<(), Box<dyn Error>> {
    let src = entry.dropbox_url.unwrap();
    // the ?dl=0 param at the end of each URL takes you to an interactive page
    // for viewing the folder in the browser. For some reason, curl and wget can
    // both get around this to download the file with no extra flags needed but
    // I can't figure out how to make reqwest do that so this switches it to ?dl=1
    // which redirects to a direct download link
    let src = &format!("{}{}", &src[..src.len() - 1], "1");

    println!("> download {} to {}", src, TMP_DOWNLOAD_NAME);

    let mut output =
        File::create(TMP_DOWNLOAD_NAME).expect(&format!("unable to create {}", TMP_DOWNLOAD_NAME));

    let mut resp = reqwest::get(src).await.unwrap();

    match resp.error_for_status_ref() {
        Ok(_) => {}
        Err(err) => {
            let err = format!("error getting {}: {}", src, err);
            return Err(err.into());
        }
    };
    while let Some(chunk) = resp.chunk().await.unwrap() {
        output.write_all(&chunk).unwrap();
    }

    Ok(())
}

fn unzip(path: &str) {
    println!("> unzip {} {}", TMP_DOWNLOAD_NAME, path);

    let file =
        File::open(TMP_DOWNLOAD_NAME).expect(&format!("unable to open {}", TMP_DOWNLOAD_NAME));
    let mut archive = zip::ZipArchive::new(file).unwrap();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let outpath = file.sanitized_name();

        {
            let comment = file.comment();
            if !comment.is_empty() {
                println!(">  file {} comment: {}", i, comment);
            }
        }

        if (&*file.name()).ends_with('/') {
            println!(
                ">   file {} extracted to \"{}\"",
                i,
                outpath.as_path().display()
            );
            create_dir_all(&outpath).unwrap();
        } else {
            println!(
                ">   file {} extracted to \"{}\"",
                i,
                outpath.as_path().display(),
            );
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    create_dir_all(&p).unwrap();
                }
            }
            let mut outfile = File::create(&outpath).unwrap();
            copy(&mut file, &mut outfile).unwrap();
        }

        // Get and Set permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            if let Some(mode) = file.unix_mode() {
                set_permissions(&outpath, Permissions::from_mode(mode)).unwrap();
            }
        }
    }
}
