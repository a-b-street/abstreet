use anyhow::Result;

/// Performs an HTTP POST request and returns the respone.
pub async fn http_post<I: AsRef<str>>(url: I, body: String) -> Result<String> {
    let url = url.as_ref();
    info!("HTTP POST to {}", url);
    let resp = reqwest::Client::new()
        .post(url)
        .body(body)
        .send()
        .await?
        .text()
        .await?;
    Ok(resp)
}
