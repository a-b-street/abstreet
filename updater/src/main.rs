use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::process::Command;

use anyhow::{Context, Result};
use walkdir::WalkDir;

use abstio::{DataPacks, Entry, Manifest};
use abstutil::{must_run_cmd, prettyprint_usize, CmdArgs, Parallelism, Timer};
use geom::Percent;

const MD5_BUF_READ_SIZE: usize = 4096;

#[tokio::main]
async fn main() {
    let mut args = CmdArgs::new();
    let version = args.optional("--version").unwrap_or("dev".to_string());
    if args.enabled("--upload") {
        assert_eq!(version, "dev");
        args.done();
        upload(version);
    } else if args.enabled("--dry") {
        let single_file = args.optional_free();
        args.done();
        if let Some(path) = single_file {
            let (local, _) = md5sum(&path);
            let truth = Manifest::load()
                .entries
                .remove(&path)
                .expect(&format!("{} not in data/MANIFEST.txt", path))
                .checksum;
            if local != truth {
                println!("{} has changed", path);
            }
        } else {
            just_compare();
        }
    } else if args.enabled("--opt-into-all") {
        args.done();
        opt_into_all();
    } else {
        let quiet = args.enabled("--quiet");
        args.done();
        download(version, quiet).await;
    }
}

async fn download(version: String, quiet: bool) {
    let data_packs = DataPacks::load_or_create();
    let local = generate_manifest();
    let truth = Manifest::load().filter(data_packs);

    // Anything local need deleting?
    for path in local.entries.keys() {
        if !truth.entries.contains_key(path) {
            rm(&path);
        }
    }

    // Anything missing or needing updating?
    let mut failed = Vec::new();
    for (path, entry) in truth.entries {
        if local.entries.get(&path).map(|x| &x.checksum) != Some(&entry.checksum) {
            std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
            match curl(&version, &path, quiet).await {
                Ok(bytes) => {
                    println!(
                        "> decompress {}, which is {} bytes compressed",
                        path,
                        prettyprint_usize(bytes.len())
                    );
                    let mut decoder = flate2::read::GzDecoder::new(&bytes[..]);
                    let mut out = File::create(&path).unwrap();
                    if let Err(err) = std::io::copy(&mut decoder, &mut out) {
                        println!("{}, but continuing", err);
                        failed.push(format!("{} failed: {}", path, err));
                    }
                }
                Err(err) => {
                    println!("{}, but continuing", err);
                    failed.push(format!("{} failed: {}", path, err));
                }
            };
        }
    }
    if !failed.is_empty() {
        // Fail the build.
        panic!("Failed to download stuff: {:?}", failed);
    }

    remove_empty_directories("data/input");
    remove_empty_directories("data/system");
}

fn just_compare() {
    let data_packs = DataPacks::load_or_create();
    let local = generate_manifest();
    let truth = Manifest::load().filter(data_packs);

    // Anything local need deleting?
    for path in local.entries.keys() {
        if !truth.entries.contains_key(path) {
            println!("- Remove {}", path);
        }
    }

    // Anything missing or needing updating?
    for (path, entry) in truth.entries {
        if local.entries.get(&path).map(|x| &x.checksum) != Some(&entry.checksum) {
            println!("- Update {}", path);
        }
    }
}

fn upload(version: String) {
    let remote_base = format!("/home/dabreegster/s3_abst_data/{}", version);

    let mut local = generate_manifest();
    let remote: Manifest = abstio::maybe_read_json(
        format!("{}/MANIFEST.json", remote_base),
        &mut Timer::throwaway(),
    )
    .unwrap_or(Manifest {
        entries: BTreeMap::new(),
    });

    // Anything remote need deleting?
    for path in remote.entries.keys() {
        if !local.entries.contains_key(path) {
            rm(&format!("{}/{}.gz", remote_base, path));
        }
    }

    // Anything missing or needing updating?
    let local_entries = std::mem::replace(&mut local.entries, BTreeMap::new());
    for (path, entry) in Timer::new("compress files").parallelize(
        "compress files",
        Parallelism::Fastest,
        local_entries.into_iter().collect(),
        |(path, mut entry)| {
            let remote_path = format!("{}/{}.gz", remote_base, path);
            let changed = remote.entries.get(&path).map(|x| &x.checksum) != Some(&entry.checksum);
            if changed {
                std::fs::create_dir_all(std::path::Path::new(&remote_path).parent().unwrap())
                    .unwrap();
                println!("> compressing {}", path);
                {
                    let mut input = BufReader::new(File::open(&path).unwrap());
                    let out = File::create(&remote_path).unwrap();
                    let mut encoder =
                        flate2::write::GzEncoder::new(out, flate2::Compression::best());
                    std::io::copy(&mut input, &mut encoder).unwrap();
                    encoder.finish().unwrap();
                }
            }
            // Always do this -- even if nothing changed, compressed_size_bytes isn't filled out by
            // generate_manifest.
            entry.compressed_size_bytes = std::fs::metadata(&remote_path).unwrap().len() as usize;
            (path, entry)
        },
    ) {
        local.entries.insert(path, entry);
    }

    abstio::write_json(format!("{}/MANIFEST.json", remote_base), &local);
    abstio::write_json("data/MANIFEST.json".to_string(), &local);

    must_run_cmd(
        Command::new("aws")
            .arg("s3")
            .arg("sync")
            .arg("--delete")
            .arg(format!("{}/data", remote_base))
            .arg(format!("s3://abstreet/{}/data", version)),
    );
    // Because of the directory structure, do this one separately, without --delete. The wasm files
    // also live in /dev/.
    must_run_cmd(
        Command::new("aws")
            .arg("s3")
            .arg("cp")
            .arg(format!("{}/MANIFEST.json", remote_base))
            .arg(format!("s3://abstreet/{}/MANIFEST.json", version)),
    );
}

fn opt_into_all() {
    let mut data_packs = DataPacks {
        runtime: BTreeSet::new(),
        input: BTreeSet::new(),
    };
    for path in Manifest::load().entries.keys() {
        if Manifest::is_file_part_of_huge_seattle(path) {
            data_packs.runtime.insert("us/huge_seattle".to_string());
            continue;
        }
        if path.starts_with("data/system/extra_fonts") {
            continue;
        }
        let parts = path.split("/").collect::<Vec<_>>();
        let city = format!("{}/{}", parts[2], parts[3]);
        if parts[1] == "input" {
            data_packs.input.insert(city);
        } else if parts[1] == "system" {
            data_packs.runtime.insert(city);
        }
    }
    println!("{}", abstutil::to_json(&data_packs));
}

fn generate_manifest() -> Manifest {
    let mut paths = Vec::new();
    for entry in WalkDir::new("data/input")
        .into_iter()
        .chain(WalkDir::new("data/system").into_iter())
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_dir() {
            continue;
        }
        let orig_path = entry.path().display().to_string();
        let path = orig_path.replace("\\", "/");
        if path.contains("system/assets/")
            || path.contains("system/proposals")
            || path.contains("system/study_areas")
        {
            continue;
        }
        paths.push((orig_path, path));
    }

    let mut kv = BTreeMap::new();
    for (path, entry) in Timer::new("compute md5sums").parallelize(
        "compute md5sums",
        Parallelism::Fastest,
        paths,
        |(orig_path, path)| {
            let (checksum, uncompressed_size_bytes) = md5sum(&orig_path);
            (
                path,
                Entry {
                    checksum,
                    uncompressed_size_bytes,
                    // Will calculate later
                    compressed_size_bytes: 0,
                },
            )
        },
    ) {
        kv.insert(path, entry);
    }

    Manifest { entries: kv }
}

/// Returns (checksum, uncompressed_size_bytes)
fn md5sum(path: &str) -> (String, usize) {
    // since these files can be very large, computes the md5 hash in chunks
    let mut file = File::open(path).unwrap();
    let mut buffer = [0 as u8; MD5_BUF_READ_SIZE];
    let mut context = md5::Context::new();
    let mut uncompressed_size_bytes = 0;
    while let Ok(n) = file.read(&mut buffer) {
        if n == 0 {
            break;
        }
        uncompressed_size_bytes += n;
        context.consume(&buffer[..n]);
    }
    (format!("{:x}", context.compute()), uncompressed_size_bytes)
}

fn rm(path: &str) {
    println!("> rm {}", path);
    match std::fs::remove_file(path) {
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

async fn curl(version: &str, path: &str, quiet: bool) -> Result<Vec<u8>> {
    // Manually enable to "download" from my local copy
    if false {
        return abstio::slurp_file(format!(
            "/home/dabreegster/s3_abst_data/{}/{}.gz",
            version, path
        ));
    }

    let src = format!(
        "http://abstreet.s3-website.us-east-2.amazonaws.com/{}/{}.gz",
        version, path
    );
    println!("> download {}", src);

    let mut resp = reqwest::get(&src).await.unwrap();
    resp.error_for_status_ref()
        .with_context(|| format!("downloading {}", src))?;

    let total_size = resp.content_length().map(|x| x as usize);
    let mut bytes = Vec::new();
    while let Some(chunk) = resp.chunk().await.unwrap() {
        if let Some(n) = total_size {
            if !quiet {
                abstutil::clear_current_line();
                print!(
                    "{} ({} / {} bytes)",
                    Percent::of(bytes.len(), n),
                    prettyprint_usize(bytes.len()),
                    prettyprint_usize(n)
                );
            }
        }

        bytes.write_all(&chunk).unwrap();
    }
    println!();
    Ok(bytes)
}

// download() will remove stray files, but leave empty directories around. Since some runtime code
// discovers lists of countries, cities, etc from the filesystem, this can get confusing.
//
// I'm sure there's a simpler way to do this, but I haven't found it.
fn remove_empty_directories(root: &str) {
    loop {
        // First just find all directories and files.
        let mut all_paths = Vec::new();
        let mut all_dirs = Vec::new();
        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path().display().to_string();
            all_paths.push(path.clone());
            if entry.file_type().is_dir() {
                all_dirs.push(path);
            }
        }

        // Now filter out directories that're a prefix of some path.
        all_dirs.retain(|dir| !all_paths.iter().any(|p| p != dir && p.starts_with(dir)));

        if all_dirs.is_empty() {
            break;
        } else {
            // Remove them! Then repeat, since we might have nested/empty/directories/.
            for x in all_dirs {
                println!("> Removing empty directory {}", x);
                // This fails if the directory isn't empty, which is a good sanity check. If
                // something weird happened, just bail.
                std::fs::remove_dir(&x).unwrap();
            }
        }
    }
}
