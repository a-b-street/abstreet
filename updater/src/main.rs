use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::process::Command;

use anyhow::{Context, Result};
use walkdir::WalkDir;

use abstio::{DataPacks, Entry, Manifest};
use abstutil::{prettyprint_usize, CmdArgs, Timer};
use geom::Percent;

const MD5_BUF_READ_SIZE: usize = 4096;

#[tokio::main]
async fn main() {
    let mut args = CmdArgs::new();
    let version = args.optional("--version").unwrap_or("dev".to_string());
    if args.enabled("--upload") {
        assert_eq!(version, "dev");
        upload(version);
        return;
    }
    if args.enabled("--dry") {
        just_compare();
        return;
    }
    let quiet = args.enabled("--quiet");
    args.done();
    download(version, quiet).await;
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
    for (path, entry) in &mut local.entries {
        let remote_path = format!("{}/{}.gz", remote_base, path);
        let changed = remote.entries.get(path).map(|x| &x.checksum) != Some(&entry.checksum);
        if changed {
            std::fs::create_dir_all(std::path::Path::new(&remote_path).parent().unwrap()).unwrap();
            println!("> compressing {}", path);
            {
                let mut input = BufReader::new(File::open(&path).unwrap());
                let out = File::create(&remote_path).unwrap();
                let mut encoder = flate2::write::GzEncoder::new(out, flate2::Compression::best());
                std::io::copy(&mut input, &mut encoder).unwrap();
                encoder.finish().unwrap();
            }
        }
        // Always do this -- even if nothing changed, compressed_size_bytes isn't filled out by
        // generate_manifest.
        entry.compressed_size_bytes = std::fs::metadata(&remote_path).unwrap().len() as usize;
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

fn generate_manifest() -> Manifest {
    let mut kv = BTreeMap::new();
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
        if path.contains("system/assets/") || path.contains("system/proposals") {
            continue;
        }

        abstutil::clear_current_line();
        print!("> compute md5sum of {}", path);
        std::io::stdout().flush().unwrap();

        // since these files can be very large, computes the md5 hash in chunks
        let mut file = File::open(&orig_path).unwrap();
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
        let checksum = format!("{:x}", context.compute());
        kv.insert(
            path,
            Entry {
                checksum,
                uncompressed_size_bytes,
                // Will calculate later
                compressed_size_bytes: 0,
            },
        );
    }
    println!();
    Manifest { entries: kv }
}

fn must_run_cmd(cmd: &mut Command) {
    println!("> Running {:?}", cmd);
    match cmd.status() {
        Ok(status) => {
            if !status.success() {
                panic!("{:?} failed", cmd);
            }
        }
        Err(err) => {
            panic!("Failed to run {:?}: {:?}", cmd, err);
        }
    }
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
        let path = format!("/home/dabreegster/s3_abst_data/{}/{}.gz", version, path);
        return abstio::slurp_file(&path);
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
