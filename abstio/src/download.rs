use std::io::Write;

use anyhow::{Context, Result};
use futures_channel::mpsc;

use abstutil::prettyprint_usize;

/// Downloads bytes from a URL. This must be called with a tokio runtime somewhere. The caller
/// creates an mpsc channel pair and provides the sender. Progress will be described through it.
pub async fn download_bytes<I: AsRef<str>>(
    url: I,
    post_body: Option<String>,
    progress: &mut mpsc::Sender<String>,
) -> Result<Vec<u8>> {
    let url = url.as_ref();
    info!("Downloading {}", url);
    let mut resp = if let Some(body) = post_body {
        reqwest::Client::new()
            .post(url)
            .body(body)
            .send()
            .await
            .unwrap()
    } else {
        reqwest::get(url).await.unwrap()
    };
    resp.error_for_status_ref()
        .with_context(|| format!("downloading {}", url))?;

    let total_size = resp.content_length().map(|x| x as usize);
    let mut bytes = Vec::new();
    while let Some(chunk) = resp.chunk().await.unwrap() {
        // TODO Throttle?
        let msg = if let Some(n) = total_size {
            format!(
                "{:.2}% ({} / {} bytes)",
                (bytes.len() as f64) / (n as f64) * 100.0,
                prettyprint_usize(bytes.len()),
                prettyprint_usize(n)
            )
        } else {
            // One example where the HTTP response won't say the response size is the Overpass API
            format!(
                "{} bytes (unknown total size)",
                prettyprint_usize(bytes.len())
            )
        };
        if let Err(err) = progress.try_send(msg) {
            warn!("Couldn't send download progress message: {}", err);
        }

        bytes.write_all(&chunk).unwrap();
    }
    println!();
    Ok(bytes)
}

/// Download a file from a URL. This must be called with a tokio runtime somewhere. Progress will
/// be printed to STDOUT.
pub async fn download_to_file<I1: AsRef<str>, I2: AsRef<str>>(
    url: I1,
    post_body: Option<String>,
    path: I2,
) -> Result<()> {
    let (mut tx, rx) = futures_channel::mpsc::channel(1000);
    print_download_progress(rx);
    let bytes = download_bytes(url, post_body, &mut tx).await?;
    let path = path.as_ref();
    std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap())?;
    let mut file = std::fs::File::create(path)?;
    file.write_all(&bytes)?;
    Ok(())
}

/// Print download progress to STDOUT. Pass this the receiver, then call download_to_file or
/// download_bytes with the sender.
pub fn print_download_progress(mut progress: mpsc::Receiver<String>) {
    tokio::task::spawn_blocking(move || loop {
        match progress.try_next() {
            Ok(Some(msg)) => {
                abstutil::clear_current_line();
                print!("{}", msg);
                std::io::stdout().flush().unwrap();
            }
            Ok(None) => break,
            // Per
            // https://docs.rs/futures-channel/0.3.14/futures_channel/mpsc/struct.Receiver.html#method.try_next,
            // this means no messages are available yet
            Err(_) => {}
        }
    });
}
