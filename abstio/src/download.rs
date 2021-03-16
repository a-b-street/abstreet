use std::io::{stdout, Write};

use anyhow::{Context, Result};

use abstutil::prettyprint_usize;

/// Downloads bytes from a URL. If `quiet` is false, prints progress. This must be called with a
/// tokio runtime somewhere.
pub async fn download_bytes<I: AsRef<str>>(url: I, quiet: bool) -> Result<Vec<u8>> {
    let url = url.as_ref();
    let mut resp = reqwest::get(url).await.unwrap();
    resp.error_for_status_ref()
        .with_context(|| format!("downloading {}", url))?;

    let total_size = resp.content_length().map(|x| x as usize);
    let mut bytes = Vec::new();
    while let Some(chunk) = resp.chunk().await.unwrap() {
        if let Some(n) = total_size {
            if !quiet {
                abstutil::clear_current_line();
                print!(
                    "{:.2}% ({} / {} bytes)",
                    (bytes.len() as f64) / (n as f64) * 100.0,
                    prettyprint_usize(bytes.len()),
                    prettyprint_usize(n)
                );
                stdout().flush().unwrap();
            }
        }

        bytes.write_all(&chunk).unwrap();
    }
    if !quiet {
        println!();
    }
    Ok(bytes)
}

/// Downloads a file. If `quiet` is false, prints progress. This must be called with a tokio
/// runtime somewhere.
pub async fn download_to_file<I: AsRef<str>>(url: I, path: String, quiet: bool) -> Result<()> {
    let bytes = download_bytes(url, quiet).await?;
    std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap())?;
    let mut file = std::fs::File::create(&path)?;
    file.write_all(&bytes)?;
    Ok(())
}
