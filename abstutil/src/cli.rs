use std::ffi::OsString;

/// Returns arguments passed in from the command-line, starting with the program name.
///
/// On web, this transforms URL query parameters into command-line arguments, by treating `&` as
/// the separator between arguments. So for instance "?--dev&--color_scheme=night%20mode" becomes
/// vec!["dummy program name", "--dev", "--color_scheme=night mode"].
pub fn cli_args() -> impl Iterator<Item = OsString> {
    #[cfg(target_arch = "wasm32")]
    {
        match parse_args() {
            Ok(mut args) => {
                args.insert(0, "dummy program name".to_string());
                let x: Vec<OsString> = args.into_iter().map(|x| x.into()).collect();
                return x.into_iter();
            }
            Err(err) => {
                warn!("Didn't parse arguments from URL query params: {}", err);
                Vec::new().into_iter()
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::args_os()
    }
}

/// Transforms some command-line arguments into URL query parameters, using `&` as the separator
/// between arguments. The string returned starts with `?`, unless the arguments are all empty.
pub fn args_to_query_string(args: Vec<String>) -> String {
    // TODO Similar to parse_args forgoing a URL decoding crate, just handle this one
    // transformation
    let result = args
        .into_iter()
        .map(|x| x.replace(" ", "%20"))
        .collect::<Vec<_>>()
        .join("&");
    if result.is_empty() {
        result
    } else {
        format!("?{}", result)
    }
}

#[cfg(target_arch = "wasm32")]
fn parse_args() -> anyhow::Result<Vec<String>> {
    let window = web_sys::window().ok_or(anyhow!("no window?"))?;
    let url = window.location().href().map_err(|err| {
        anyhow!(err
            .as_string()
            .unwrap_or("window.location.href failed".to_string()))
    })?;
    // Consider using a proper url parsing crate. This works fine for now, though.
    let url_parts = url.split("?").collect::<Vec<_>>();
    if url_parts.len() != 2 {
        bail!("URL {url} doesn't seem to have query params");
    }
    let parts = url_parts[1]
        .split("&")
        .map(|x| x.replace("%20", " ").to_string())
        .collect::<Vec<_>>();
    Ok(parts)
}
