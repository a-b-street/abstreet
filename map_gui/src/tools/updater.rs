use std::collections::BTreeSet;
use std::fs::File;

use futures_channel::mpsc;

use abstio::{DataPacks, Manifest, MapName};
use widgetry::{EventCtx, Key, Transition};

use crate::load::FutureLoader;
use crate::tools::{ChooseSomething, PopupMsg};
use crate::AppLike;

// Update this ___before___ pushing the commit with "[rebuild] [release]".
const NEXT_RELEASE: &str = "0.2.45";

// For each city, how many total bytes do the runtime files cost to download?

/// How many bytes to download for a city?
fn size_of_city(map: &MapName) -> u64 {
    let mut data_packs = DataPacks {
        runtime: BTreeSet::new(),
        input: BTreeSet::new(),
    };
    data_packs.runtime.insert(map.to_data_pack_name());
    let mut manifest = Manifest::load().filter(data_packs);
    // Don't download files that already exist
    abstutil::retain_btreemap(&mut manifest.entries, |path, _| {
        !abstio::file_exists(&abstio::path(path.strip_prefix("data/").unwrap()))
    });
    let mut bytes = 0;
    for (_, entry) in manifest.entries {
        bytes += entry.compressed_size_bytes;
    }
    bytes
}

fn prettyprint_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{} bytes", bytes);
    }
    let kb = (bytes as f64) / 1024.0;
    if kb < 1024.0 {
        return format!("{} KB", kb as usize);
    }
    let mb = kb / 1024.0;
    format!("{} MB", mb as usize)
}

/// Prompt to download a missing city. On either success or failure (maybe the player choosing to
/// not download, maybe a network error), the new map isn't automatically loaded or anything; up to
/// the caller to handle that.
pub fn prompt_to_download_missing_data<A: AppLike + 'static>(
    ctx: &mut EventCtx,
    map_name: MapName,
) -> Transition<A> {
    Transition::Push(ChooseSomething::new_state(
        ctx,
        format!(
            "Missing data. Download {} for {}?",
            prettyprint_bytes(size_of_city(&map_name)),
            map_name.city.describe()
        ),
        vec![
            widgetry::Choice::string("Yes, download"),
            widgetry::Choice::string("Never mind").key(Key::Escape),
        ],
        Box::new(move |resp, ctx, _| {
            if resp == "Never mind" {
                return Transition::Pop;
            }

            let cities = vec![map_name.to_data_pack_name()];

            // Adjust the updater's config, in case the user also runs that.
            let mut packs = DataPacks::load_or_create();
            packs.runtime.insert(cities[0].clone());
            packs.save();

            let (outer_progress_tx, outer_progress_rx) = futures_channel::mpsc::channel(1000);
            let (inner_progress_tx, inner_progress_rx) = futures_channel::mpsc::channel(1000);
            Transition::Replace(FutureLoader::<A, Vec<String>>::new_state(
                ctx,
                Box::pin(async {
                    let result =
                        download_cities(cities, outer_progress_tx, inner_progress_tx).await;
                    let wrap: Box<dyn Send + FnOnce(&A) -> Vec<String>> =
                        Box::new(move |_: &A| result);
                    Ok(wrap)
                }),
                outer_progress_rx,
                inner_progress_rx,
                "Downloading missing files",
                Box::new(|ctx, _, maybe_messages| {
                    let messages = match maybe_messages {
                        Ok(m) => m,
                        Err(err) => vec![format!("Something went very wrong: {}", err)],
                    };
                    Transition::Replace(PopupMsg::new_state(
                        ctx,
                        "Download complete. Try again!",
                        messages,
                    ))
                }),
            ))
        }),
    ))
}

async fn download_cities(
    cities: Vec<String>,
    mut outer_progress: mpsc::Sender<String>,
    mut inner_progress: mpsc::Sender<String>,
) -> Vec<String> {
    let mut data_packs = DataPacks {
        runtime: BTreeSet::new(),
        input: BTreeSet::new(),
    };
    data_packs.runtime.extend(cities);
    let mut manifest = Manifest::load().filter(data_packs);
    // Don't download files that already exist
    abstutil::retain_btreemap(&mut manifest.entries, |path, _| {
        !abstio::file_exists(&abstio::path(path.strip_prefix("data/").unwrap()))
    });

    let version = if cfg!(feature = "release_s3") {
        NEXT_RELEASE
    } else {
        "dev"
    };

    let num_files = manifest.entries.len();
    let mut messages = Vec::new();
    let mut files_so_far = 0;

    for (path, entry) in manifest.entries {
        files_so_far += 1;
        let local_path = abstio::path(path.strip_prefix("data/").unwrap());
        let url = format!(
            "http://abstreet.s3-website.us-east-2.amazonaws.com/{}/{}.gz",
            version, path
        );
        if let Err(err) = outer_progress.try_send(format!(
            "Downloading file {}/{}: {} ({})",
            files_so_far,
            num_files,
            url,
            prettyprint_bytes(entry.compressed_size_bytes)
        )) {
            warn!("Couldn't send progress: {}", err);
        }

        match abstio::download_bytes(&url, &mut inner_progress)
            .await
            .and_then(|bytes| {
                // TODO Instead of holding everything in memory like this, we could also try to
                // stream the gunzipping and output writing
                info!("Decompressing {}", path);
                std::fs::create_dir_all(std::path::Path::new(&local_path).parent().unwrap())
                    .unwrap();
                let mut out = File::create(&local_path).unwrap();
                let mut decoder = flate2::read::GzDecoder::new(&bytes[..]);
                std::io::copy(&mut decoder, &mut out).map_err(|err| err.into())
            }) {
            Ok(_) => {}
            Err(err) => {
                let msg = format!("Problem with {}: {}", url, err);
                error!("{}", msg);
                messages.push(msg);
            }
        }
    }
    messages.insert(0, format!("Downloaded {} files", num_files));
    messages
}
