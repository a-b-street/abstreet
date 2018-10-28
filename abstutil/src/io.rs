use multimap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_cbor;
use serde_json;
use std;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::hash::Hash;
use std::io::{stdout, BufReader, Error, ErrorKind, Read, Write};
use std::path::Path;
use std::time::Instant;
use {elapsed_seconds, PROGRESS_FREQUENCY_SECONDS};

pub fn to_json<T: Serialize>(obj: &T) -> String {
    serde_json::to_string_pretty(obj).unwrap()
}

pub fn write_json<T: Serialize>(path: &str, obj: &T) -> Result<(), Error> {
    std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap())
        .expect("Creating parent dir failed");

    let mut file = File::create(path)?;
    file.write_all(to_json(obj).as_bytes())?;
    Ok(())
}

pub fn read_json<T: DeserializeOwned>(path: &str) -> Result<T, Error> {
    // TODO easier way to map_err for anything in a block that has ?
    inner_read_json(path).map_err(|e| Error::new(e.kind(), format!("read_json({}): {}", path, e)))
}

fn inner_read_json<T: DeserializeOwned>(path: &str) -> Result<T, Error> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let obj: T = serde_json::from_str(&contents)?;
    Ok(obj)
}

pub fn write_binary<T: Serialize>(path: &str, obj: &T) -> Result<(), Error> {
    std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap())
        .expect("Creating parent dir failed");

    let mut file = File::create(path)?;
    serde_cbor::to_writer(&mut file, obj).map_err(|err| Error::new(ErrorKind::Other, err))
}

pub fn read_binary<T: DeserializeOwned>(path: &str) -> Result<T, Error> {
    let reader = FileWithProgress::new(path)?;
    let obj: T =
        serde_cbor::from_reader(reader).map_err(|err| Error::new(ErrorKind::Other, err))?;
    Ok(obj)
}

// For BTreeMaps with struct keys. See https://github.com/serde-rs/json/issues/402.

pub fn serialize_btreemap<S: Serializer, K: Serialize, V: Serialize>(
    map: &BTreeMap<K, V>,
    s: S,
) -> Result<S::Ok, S::Error> {
    map.iter()
        .map(|(a, b)| (a.clone(), b.clone()))
        .collect::<Vec<(_, _)>>()
        .serialize(s)
}

pub fn deserialize_btreemap<
    'de,
    D: Deserializer<'de>,
    K: Deserialize<'de> + Ord,
    V: Deserialize<'de>,
>(
    d: D,
) -> Result<BTreeMap<K, V>, D::Error> {
    let vec = <Vec<(K, V)>>::deserialize(d)?;
    let mut map = BTreeMap::new();
    for (k, v) in vec {
        map.insert(k, v);
    }
    Ok(map)
}

pub fn serialize_multimap<S: Serializer, K: Serialize + Eq + Hash, V: Serialize + Eq + Hash>(
    map: &multimap::MultiMap<K, V>,
    s: S,
) -> Result<S::Ok, S::Error> {
    // TODO maybe need to sort to have deterministic output
    map.iter_all()
        .map(|(key, values)| (key.clone(), values.clone()))
        .collect::<Vec<(_, _)>>()
        .serialize(s)
}

pub fn deserialize_multimap<
    'de,
    D: Deserializer<'de>,
    K: Deserialize<'de> + Eq + Hash + Clone,
    V: Deserialize<'de> + Eq + Hash,
>(
    d: D,
) -> Result<multimap::MultiMap<K, V>, D::Error> {
    let vec = <Vec<(K, Vec<V>)>>::deserialize(d)?;
    let mut map = multimap::MultiMap::new();
    for (key, values) in vec {
        for value in values {
            map.insert(key.clone(), value);
        }
    }
    Ok(map)
}

// Just list all things from a directory, return sorted by name, with file extension removed.
// Pretty hacky that we return a (String, String).
pub fn list_all_objects(dir: &str, map_name: &str) -> Vec<(String, String)> {
    let mut results: BTreeSet<(String, String)> = BTreeSet::new();
    match std::fs::read_dir(format!("../data/{}/{}/", dir, map_name)) {
        Ok(iter) => {
            for entry in iter {
                let filename = entry.unwrap().file_name();
                let path = Path::new(&filename);
                if path.to_string_lossy().ends_with(".swp") {
                    continue;
                }
                let name = path
                    .file_stem()
                    .unwrap()
                    .to_os_string()
                    .into_string()
                    .unwrap();
                results.insert((name.clone(), name));
            }
        }
        Err(ref e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => panic!(e),
    };
    results.into_iter().collect()
}

// Load all serialized things from a directory, return sorted by name, with file extension removed.
pub fn load_all_objects<T: DeserializeOwned>(dir: &str, map_name: &str) -> Vec<(String, T)> {
    let mut tree: BTreeMap<String, T> = BTreeMap::new();
    match std::fs::read_dir(format!("../data/{}/{}/", dir, map_name)) {
        Ok(iter) => {
            for entry in iter {
                let filename = entry.unwrap().file_name();
                let path = Path::new(&filename);
                if path.to_string_lossy().ends_with(".swp") {
                    continue;
                }
                let name = path
                    .file_stem()
                    .unwrap()
                    .to_os_string()
                    .into_string()
                    .unwrap();
                let load: T =
                    read_json(&format!("../data/{}/{}/{}.json", dir, map_name, name)).unwrap();
                tree.insert(name, load);
            }
        }
        Err(ref e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => panic!(e),
    };
    tree.into_iter().collect()
}

pub fn save_object<T: Serialize>(dir: &str, map_name: &str, obj_name: &str, obj: &T) {
    let path = format!("../data/{}/{}/{}.json", dir, map_name, obj_name);
    write_json(&path, obj).expect(&format!("Saving {} failed", path));
    println!("Saved {}", path);
}

struct FileWithProgress {
    inner: BufReader<File>,

    path: String,
    processed_bytes: usize,
    total_bytes: usize,
    started_at: Instant,
    last_printed_at: Instant,
}

impl FileWithProgress {
    pub fn new(path: &str) -> Result<FileWithProgress, Error> {
        let file = File::open(path)?;
        let total_bytes = file.metadata()?.len() as usize;
        Ok(FileWithProgress {
            inner: BufReader::new(file),
            path: path.to_string(),
            processed_bytes: 0,
            total_bytes,
            started_at: Instant::now(),
            last_printed_at: Instant::now(),
        })
    }
}

impl Read for FileWithProgress {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let bytes = self.inner.read(buf)?;
        self.processed_bytes += bytes;
        if self.processed_bytes > self.total_bytes {
            panic!(
                "{} is too many bytes read from {}",
                self.processed_bytes, self.path
            );
        }

        let done = self.processed_bytes == self.total_bytes && bytes == 0;
        if elapsed_seconds(self.last_printed_at) >= PROGRESS_FREQUENCY_SECONDS || done {
            self.last_printed_at = Instant::now();
            print!(
                "{}Reading {}: {}/{} MB... {}s",
                "\r",
                self.path,
                self.processed_bytes / 1024 / 1024,
                self.total_bytes / 1024 / 1024,
                elapsed_seconds(self.started_at)
            );
            if done {
                println!("");
            } else {
                stdout().flush().unwrap();
            }
        }

        Ok(bytes)
    }
}
