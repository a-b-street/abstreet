use std::collections::BTreeMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};

// keyed by path
pub struct Manifest(pub BTreeMap<String, Entry>);
pub struct Entry {
    pub checksum: String,
    pub dropbox_url: Option<String>,
}

impl Manifest {
    pub fn write(&self, path: String) {
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

    pub fn load(path: String) -> Result<Manifest, Box<dyn Error>> {
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
