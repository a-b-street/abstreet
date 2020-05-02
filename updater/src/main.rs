use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Error, Write};
use std::process::Command;
use walkdir::WalkDir;

fn main() {
    upload()
}

fn upload() {
    let remote_base = "/home/dabreegster/Dropbox/abstreet_data";

    let local = Manifest::generate();
    local.write("data/MANIFEST.txt".to_string());

    let remote = Manifest::load(format!("{}/MANIFEST.txt", remote_base))
        .unwrap_or(Manifest(BTreeMap::new()));
    // Anything remote need deleting?
    for file in remote.0.keys() {
        if !local.0.contains_key(file) {
            let path = format!("{}/{}.zip", remote_base, file);
            run(Command::new("rm").arg(path));
        }
    }
    // Anything missing or needing updating?
    for (file, checksum) in &local.0 {
        if remote.0.get(file) != Some(checksum) {
            let path = format!("{}/{}.zip", remote_base, file);
            std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
            run(Command::new("zip").arg(path).arg(file));
        }
    }
    // "Commit" the remote result by writing this
    local.write(format!("{}/MANIFEST.txt", remote_base));
}

// path -> checksum
struct Manifest(BTreeMap<String, String>);

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
            if path.contains("system/assets/") || path.contains("system/fonts") {
                continue;
            }
            let checksum = run(Command::new("md5sum").arg(&path))
                .split(" ")
                .next()
                .unwrap()
                .to_string();
            kv.insert(path, checksum);
        }
        Manifest(kv)
    }

    fn write(&self, path: String) {
        let mut f = File::create(&path).unwrap();
        for (file, checksum) in &self.0 {
            writeln!(f, "{} {}", checksum, file).unwrap();
        }
        println!("- Wrote {}", path);
    }

    fn load(path: String) -> Result<Manifest, Error> {
        let mut kv = BTreeMap::new();
        for line in BufReader::new(File::open(path)?).lines() {
            let line = line?;
            let parts = line.splitn(2, " ").collect::<Vec<_>>();
            kv.insert(parts[1].to_string(), parts[0].to_string());
        }
        Ok(Manifest(kv))
    }
}

fn run(cmd: &mut Command) -> String {
    println!("> {:?}", cmd);
    String::from_utf8(cmd.output().unwrap().stdout).unwrap()
}
