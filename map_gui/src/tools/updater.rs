use std::collections::BTreeSet;

use anyhow::Result;
use fs_err::File;
use futures_channel::mpsc;

use abstio::{DataPacks, Manifest, MapName};
use abstutil::prettyprint_bytes;
use widgetry::tools::{FutureLoader, PopupMsg};
use widgetry::{EventCtx, Key, Transition};

use crate::tools::ChooseSomething;
use crate::AppLike;

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
    manifest
        .entries
        .retain(|path, _| !abstio::file_exists(&abstio::path(path.strip_prefix("data/").unwrap())));
    let mut bytes = 0;
    for (_, entry) in manifest.entries {
        bytes += entry.compressed_size_bytes;
    }
    bytes
}

pub fn prompt_to_download_missing_data<A: AppLike + 'static>(
    ctx: &mut EventCtx,
    map_name: MapName,
    on_load: Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>,
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
            Transition::Replace(FutureLoader::<A, Result<()>>::new_state(
                ctx,
                Box::pin(async {
                    let result =
                        download_cities(cities, outer_progress_tx, inner_progress_tx).await;
                    let wrap: Box<dyn Send + FnOnce(&A) -> Result<()>> =
                        Box::new(move |_: &A| result);
                    Ok(wrap)
                }),
                outer_progress_rx,
                inner_progress_rx,
                "Downloading missing files",
                Box::new(|ctx, app, maybe_result| {
                    let error_msg = match maybe_result {
                        Ok(Ok(())) => None,
                        Ok(Err(err)) => Some(err.to_string()),
                        Err(err) => Some(format!("Something went very wrong: {}", err)),
                    };
                    if let Some(err) = error_msg {
                        Transition::Replace(PopupMsg::new_state(ctx, "Download failed", vec![err]))
                    } else {
                        on_load(ctx, app)
                    }
                }),
            ))
        }),
    ))
}

async fn download_cities(
    cities: Vec<String>,
    mut outer_progress: mpsc::Sender<String>,
    mut inner_progress: mpsc::Sender<String>,
) -> Result<()> {
    let mut data_packs = DataPacks {
        runtime: BTreeSet::new(),
        input: BTreeSet::new(),
    };
    data_packs.runtime.extend(cities);
    let mut manifest = Manifest::load().filter(data_packs);
    // Don't download files that already exist
    manifest
        .entries
        .retain(|path, _| !abstio::file_exists(&abstio::path(path.strip_prefix("data/").unwrap())));

    let num_files = manifest.entries.len();
    let mut messages = Vec::new();
    let mut files_so_far = 0;

    for (path, entry) in manifest.entries {
        files_so_far += 1;
        let local_path = abstio::path(path.strip_prefix("data/").unwrap());
        let url = format!(
            "http://play.abstreet.org/{}/{}.gz",
            crate::tools::version(),
            path
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

        match abstio::download_bytes(&url, None, &mut inner_progress)
            .await
            .and_then(|bytes| {
                // TODO Instead of holding everything in memory like this, we could also try to
                // stream the gunzipping and output writing
                info!("Decompressing {}", path);
                fs_err::create_dir_all(std::path::Path::new(&local_path).parent().unwrap())
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
    if !messages.is_empty() {
        bail!("{} errors: {}", messages.len(), messages.join(", "));
    }
    Ok(())
}
