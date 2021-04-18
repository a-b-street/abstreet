use std::io::Write;

use anyhow::{Context, Result};
use futures_channel::mpsc;

use abstutil::prettyprint_usize;

/// Downloads bytes from a URL. This must be called with a tokio runtime somewhere. The caller
/// creates an mpsc channel pair and provides the sender. Progress will be described through it.
pub async fn download_bytes<I: AsRef<str>>(
    url: I,
    mut progress: mpsc::Sender<String>,
) -> Result<Vec<u8>> {
    let url = url.as_ref();
    info!("Downloading {}", url);
    let mut resp = reqwest::get(url).await.unwrap();
    resp.error_for_status_ref()
        .with_context(|| format!("downloading {}", url))?;

    let total_size = resp.content_length().map(|x| x as usize);
    let mut bytes = Vec::new();
    while let Some(chunk) = resp.chunk().await.unwrap() {
        if let Some(n) = total_size {
            // TODO Throttle?
            if let Err(err) = progress.try_send(format!(
                "{:.2}% ({} / {} bytes)",
                (bytes.len() as f64) / (n as f64) * 100.0,
                prettyprint_usize(bytes.len()),
                prettyprint_usize(n)
            )) {
                warn!("Couldn't send download progress message: {}", err);
            }
        }

        bytes.write_all(&chunk).unwrap();
    }
    println!();
    Ok(bytes)
}

/// Download a file from a URL. This must be called with a tokio runtime somewhere. Progress will
/// be printed to STDOUT.
pub async fn download_to_file<I1: AsRef<str>, I2: AsRef<str>>(url: I1, path: I2) -> Result<()> {
    let (tx, rx) = futures_channel::mpsc::channel(1000);
    print_download_progress(rx);
    let bytes = download_bytes(url, tx).await?;
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
            Err(_) => {}
        }
    });
}
