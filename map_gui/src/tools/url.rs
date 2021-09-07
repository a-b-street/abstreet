use anyhow::Result;

use geom::{GPSBounds, LonLat, Pt2D};
use widgetry::EventCtx;

/// Utilities for reflecting the current map and viewport in the URL on the web. No effect on
/// native.
pub struct URLManager;

impl URLManager {
    /// This does nothing on native. On web, it modifies the current URL to change the first free
    /// parameter in the HTTP GET params to the specified value, adding it if needed.
    pub fn update_url_free_param(free_param: String) -> Result<()> {
        update_url(Box::new(move |url| change_url_free_param(url, &free_param)))
    }

    /// This does nothing on native. On web, it modifies the current URL to change the first named
    /// parameter in the HTTP GET params to the specified value, adding it if needed.
    pub fn update_url_param(key: String, value: String) -> Result<()> {
        update_url(Box::new(move |url| change_url_param(url, &key, &value)))
    }

    /// This does nothing on native. On web, it modifies the current URL to set --cam to an
    /// OSM-style `zoom/lat/lon` string
    /// (https://wiki.openstreetmap.org/wiki/Browsing#Other_URL_tricks) based on the current
    /// viewport.
    pub fn update_url_cam(ctx: &EventCtx, gps_bounds: &GPSBounds) -> Result<()> {
        let center = ctx.canvas.center_to_map_pt().to_gps(gps_bounds);

        // To calculate zoom, just solve for the inverse of the code in parse_center_camera.
        let earth_circumference_equator = 40_075_016.686;
        let log_arg =
            earth_circumference_equator * center.y().to_radians().cos() * ctx.canvas.cam_zoom;
        let zoom_lvl = log_arg.log2() - 8.0;

        // Trim precision
        let cam = format!("{:.2}/{:.5}/{:.5}", zoom_lvl, center.y(), center.x());

        update_url(Box::new(move |url| change_url_param(url, "--cam", &cam)))
    }

    /// Parse an OSM-style `zoom/lat/lon` string
    /// (https://wiki.openstreetmap.org/wiki/Browsing#Other_URL_tricks), returning the map point to
    /// center on and the camera zoom.
    pub fn parse_center_camera(raw: &str, gps_bounds: &GPSBounds) -> Option<(Pt2D, f64)> {
        let parts: Vec<&str> = raw.split('/').collect();
        if parts.len() != 3 {
            return None;
        }
        let zoom_lvl = parts[0].parse::<f64>().ok()?;
        let lat = parts[1].parse::<f64>().ok()?;
        let lon = parts[2].parse::<f64>().ok()?;
        let gps = LonLat::new(lon, lat);
        if !gps_bounds.contains(gps) {
            return None;
        }
        let pt = gps.to_pt(gps_bounds);

        // To figure out zoom, first calculate horizontal meters per pixel, using the formula from
        // https://wiki.openstreetmap.org/wiki/Zoom_levels.
        let earth_circumference_equator = 40_075_016.686;
        let horiz_meters_per_pixel =
            earth_circumference_equator * gps.y().to_radians().cos() / 2.0_f64.powf(zoom_lvl + 8.0);

        // So this is the width in meters that should cover our screen
        // let horiz_meters_per_screen = ctx.canvas.window_width * horiz_meters_per_pixel;
        // Now we want to make screen_to_map(the top-right corner of the screen) =
        // horiz_meters_per_screen. Easy algebra:
        // let cam_zoom = ctx.canvas.window_width / horiz_meters_per_screen;

        // But actually, the algebra shows we don't even need window_width. Easy!
        let cam_zoom = 1.0 / horiz_meters_per_pixel;

        Some((pt, cam_zoom))
    }
}

#[allow(unused_variables)]
fn update_url(transform: Box<dyn Fn(String) -> String>) -> Result<()> {
    #[cfg(target_arch = "wasm32")]
    {
        let window = web_sys::window().ok_or(anyhow!("no window?"))?;
        let url = window.location().href().map_err(|err| {
            anyhow!(err
                .as_string()
                .unwrap_or("window.location.href failed".to_string()))
        })?;
        let new_url = (transform)(url);

        // Setting window.location.href may seem like the obvious thing to do, but that actually
        // refreshes the page. This method just changes the URL and doesn't mess up history. See
        // https://developer.mozilla.org/en-US/docs/Web/API/History_API/Working_with_the_History_API.
        let history = window.history().map_err(|err| {
            anyhow!(err
                .as_string()
                .unwrap_or("window.history failed".to_string()))
        })?;
        history
            .replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&new_url))
            .map_err(|err| {
                anyhow!(err
                    .as_string()
                    .unwrap_or("window.history.replace_state failed".to_string()))
            })?;
    }
    Ok(())
}

fn change_url_free_param(url: String, free_param: &str) -> String {
    // The URL parsing crates I checked had lots of dependencies and didn't even expose such a nice
    // API for doing this anyway.
    let url_parts = url.split('?').collect::<Vec<_>>();
    if url_parts.len() == 1 {
        return format!("{}?{}", url, free_param);
    }
    let mut query_params = String::new();
    let mut found_free = false;
    let mut first = true;
    for x in url_parts[1].split('&') {
        if !first {
            query_params.push('&');
        }
        first = false;

        if x.starts_with("--") {
            query_params.push_str(x);
        } else if !found_free {
            // Replace the first free parameter
            query_params.push_str(free_param);
            found_free = true;
        } else {
            query_params.push_str(x);
        }
    }
    if !found_free {
        if !first {
            query_params.push('&');
        }
        query_params.push_str(free_param);
    }

    format!("{}?{}", url_parts[0], query_params)
}

fn change_url_param(url: String, key: &str, value: &str) -> String {
    // The URL parsing crates I checked had lots of dependencies and didn't even expose such a nice
    // API for doing this anyway.
    let url_parts = url.split('?').collect::<Vec<_>>();
    if url_parts.len() == 1 {
        return format!("{}?{}={}", url, key, value);
    }
    let mut query_params = String::new();
    let mut found_key = false;
    let mut first = true;
    for x in url_parts[1].split('&') {
        if !first {
            query_params.push('&');
        }
        first = false;

        if x.starts_with(key) {
            query_params.push_str(&format!("{}={}", key, value));
            found_key = true;
        } else {
            query_params.push_str(x);
        }
    }
    if !found_key {
        if !first {
            query_params.push('&');
        }
        query_params.push_str(&format!("{}={}", key, value));
    }

    format!("{}?{}", url_parts[0], query_params)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_change_url_free_param() {
        use super::change_url_free_param;

        assert_eq!(
            "http://0.0.0.0:8000/?--dev&seattle/maps/montlake.bin",
            change_url_free_param(
                "http://0.0.0.0:8000/?--dev".to_string(),
                "seattle/maps/montlake.bin"
            )
        );
        assert_eq!(
            "http://0.0.0.0:8000/?--dev&seattle/maps/qa.bin",
            change_url_free_param(
                "http://0.0.0.0:8000/?--dev&seattle/maps/montlake.bin".to_string(),
                "seattle/maps/qa.bin"
            )
        );
        assert_eq!(
            "http://0.0.0.0:8000?seattle/maps/montlake.bin",
            change_url_free_param(
                "http://0.0.0.0:8000".to_string(),
                "seattle/maps/montlake.bin"
            )
        );
    }

    #[test]
    fn test_change_url_param() {
        use super::change_url_param;

        assert_eq!(
            "http://0.0.0.0:8000/?--dev&seattle/maps/montlake.bin&--cam=16.6/53.78449/-1.70701",
            change_url_param(
                "http://0.0.0.0:8000/?--dev&seattle/maps/montlake.bin".to_string(),
                "--cam",
                "16.6/53.78449/-1.70701"
            )
        );
        assert_eq!(
            "http://0.0.0.0:8000?--cam=16.6/53.78449/-1.70701",
            change_url_param(
                "http://0.0.0.0:8000".to_string(),
                "--cam",
                "16.6/53.78449/-1.70701"
            )
        );
    }
}
