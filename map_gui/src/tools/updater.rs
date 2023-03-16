use std::collections::BTreeSet;

use anyhow::Result;
use fs_err::File;
use futures_channel::mpsc;

use abstio::{DataPacks, Manifest, MapName};
use abstutil::prettyprint_bytes;
use widgetry::tools::{ChooseSomething, FutureLoader, PopupMsg};
use widgetry::{EventCtx, Key, Transition};

use crate::AppLike;

pub fn prompt_to_download_missing_data<A: AppLike + 'static>(
    ctx: &mut EventCtx,
    map_name: MapName,
    on_load: Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>,
) -> Transition<A> {
    let manifest = files_to_download(&map_name);
    let bytes = manifest
        .entries
        .iter()
        .map(|(_, e)| e.compressed_size_bytes)
        .sum();

    Transition::Push(ChooseSomething::new_state(
        ctx,
        format!(
            "Missing data. Download {} for {}?",
            prettyprint_bytes(bytes),
            map_name.describe()
        ),
        vec![
            widgetry::Choice::string("Yes, download"),
            widgetry::Choice::string("Never mind").key(Key::Escape),
        ],
        Box::new(move |resp, ctx, _| {
            if resp == "Never mind" {
                return Transition::Pop;
            }

            let (outer_progress_tx, outer_progress_rx) = futures_channel::mpsc::channel(1000);
            let (inner_progress_tx, inner_progress_rx) = futures_channel::mpsc::channel(1000);
            Transition::Replace(FutureLoader::<A, Result<()>>::new_state(
                ctx,
                Box::pin(async {
                    let result =
                        download_files(manifest, outer_progress_tx, inner_progress_tx).await;
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

fn files_to_download(map: &MapName) -> Manifest {
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

    // DataPacks are an updater tool concept, but we don't want everything from that city, just the
    // one map (and it's scenarios, prebaked_results, and maybe the city.bin overview)
    manifest.entries.retain(|path, _| {
        // TODO This reinvents a bit of abst_data.rs
        let parts = path.split('/').collect::<Vec<_>>();
        parts[4] == "city.bin"
            || (parts[4] == "maps" && parts[5] == format!("{}.bin", map.map))
            || (parts.len() >= 6 && parts[5] == map.map)
    });

    manifest
}

async fn download_files(
    manifest: Manifest,
    mut outer_progress: mpsc::Sender<String>,
    mut inner_progress: mpsc::Sender<String>,
) -> Result<()> {
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
