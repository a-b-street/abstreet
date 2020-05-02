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
    let local = Manifest::generate();
    let truth = Manifest::load("data/MANIFEST.txt".to_string()).unwrap();

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
            run(Command::new("unzip").arg("tmp_download.zip").arg(path));
            run(Command::new("rm").arg("tmp_download.zip"));
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
            entry.dropbox_url = Some(
                run(Command::new("dropbox").arg("sharelink").arg(remote_path))
                    .trim()
                    .to_string(),
            );
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
                || path.contains("system/synthetic_maps")
            {
                continue;
            }
            let checksum = run(Command::new("md5sum").arg(&path))
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
}

fn run(cmd: &mut Command) -> String {
    println!("> {:?}", cmd);
    String::from_utf8(cmd.output().unwrap().stdout).unwrap()
}
