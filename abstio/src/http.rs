use anyhow::{Context, Result};

/// Performs an HTTP POST request and returns the response.
pub async fn http_post<U: AsRef<str>, B: Into<reqwest::Body>>(url: U, body: B) -> Result<String> {
    let url = url.as_ref();
    info!("HTTP POST to {}", url);
    let resp = reqwest::Client::new()
        .post(url)
        .body(body)
        .send()
        .await
        .with_context(|| url.to_string())?;
    let status = resp.status();
    let text = resp.text().await.with_context(|| url.to_string())?;
    // With error_for_status{_ref}, it's unclear how to propagate errors and also get the error
    // message from the body, so do this
    if status.is_client_error() || status.is_server_error() {
        bail!("HTTP error {}: {}", status, text);
    }
    Ok(text)
}

/// Performs an HTTP GET request and returns the raw response. Unlike the variations in
/// download.rs, no progress -- but it works on native and web.
pub async fn http_get<I: AsRef<str>>(url: I) -> Result<Vec<u8>> {
    let url = url.as_ref();
    info!("HTTP GET {}", url);
    let resp = reqwest::get(url).await?.error_for_status()?.bytes().await?;
    Ok(resp.to_vec())
}
