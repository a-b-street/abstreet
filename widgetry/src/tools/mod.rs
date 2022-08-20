mod choose_something;
mod colors;
mod lasso;
mod load;
mod popup;
mod prompt_input;
pub(crate) mod screenshot;
mod url;
pub(crate) mod warper;

use anyhow::Result;

pub use choose_something::ChooseSomething;
pub use colors::{ColorLegend, ColorScale, DivergingScale};
pub use lasso::{Lasso, PolyLineLasso};
pub use load::{FileLoader, FutureLoader, RawBytes};
pub use popup::PopupMsg;
pub use prompt_input::PromptInput;
pub use url::URLManager;

use crate::{Color, GfxCtx};
use geom::Polygon;

/// Store a cached key/value pair, only recalculating when the key changes.
pub struct Cached<K: PartialEq + Clone, V> {
    contents: Option<(K, V)>,
}

impl<K: PartialEq + Clone, V> Cached<K, V> {
    pub fn new() -> Cached<K, V> {
        Cached { contents: None }
    }

    /// Get the current key.
    pub fn key(&self) -> Option<K> {
        self.contents.as_ref().map(|(k, _)| k.clone())
    }

    /// Get the current value.
    pub fn value(&self) -> Option<&V> {
        self.contents.as_ref().map(|(_, v)| v)
    }

    /// Get the current value, mutably.
    pub fn value_mut(&mut self) -> Option<&mut V> {
        self.contents.as_mut().map(|(_, v)| v)
    }

    /// Update the value if the key has changed.
    pub fn update<F: FnMut(K) -> V>(&mut self, key: Option<K>, mut produce_value: F) {
        if let Some(new_key) = key {
            if self.key() != Some(new_key.clone()) {
                self.contents = Some((new_key.clone(), produce_value(new_key)));
            }
        } else {
            self.contents = None;
        }
    }

    /// `update` is preferred, but sometimes `produce_value` needs to borrow the same struct that
    /// owns this `Cached`. In that case, the caller can manually check `key` and call this.
    pub fn set(&mut self, key: K, value: V) {
        self.contents = Some((key, value));
    }

    pub fn clear(&mut self) {
        self.contents = None;
    }

    /// Clears the current pair and returns it.
    pub fn take(&mut self) -> Option<(K, V)> {
        self.contents.take()
    }
}

impl<K: PartialEq + Clone, V> Default for Cached<K, V> {
    fn default() -> Self {
        Cached::new()
    }
}

pub fn open_browser<I: AsRef<str>>(url: I) {
    let _ = webbrowser::open(url.as_ref());
}

fn grey_out_map(g: &mut GfxCtx) {
    // This is a copy of grey_out_map from map_gui, with no dependencies on App
    g.fork_screenspace();
    g.draw_polygon(
        Color::BLACK.alpha(0.6),
        Polygon::rectangle(g.canvas.window_width, g.canvas.window_height),
    );
    g.unfork();
}

/// Only works on native
pub fn set_clipboard(x: String) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use clipboard::{ClipboardContext, ClipboardProvider};
        if let Err(err) =
            ClipboardProvider::new().and_then(|mut ctx: ClipboardContext| ctx.set_contents(x))
        {
            error!("Copying to clipboard broke: {}", err);
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = x;
    }
}

pub fn get_clipboard() -> Result<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use clipboard::{ClipboardContext, ClipboardProvider};
        // TODO The clipboard crate uses old nightly Errors. Converting to anyhow is weird.
        let mut ctx: ClipboardContext = match ClipboardProvider::new() {
            Ok(ctx) => ctx,
            Err(err) => bail!("{}", err),
        };
        let contents = match ctx.get_contents() {
            Ok(contents) => contents,
            Err(err) => bail!("{}", err),
        };
        Ok(contents)
    }

    #[cfg(target_arch = "wasm32")]
    {
        bail!("Unsupported on web");
    }
}
