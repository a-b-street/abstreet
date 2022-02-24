mod lasso;
mod load;
mod popup;
pub(crate) mod screenshot;
mod url;
pub(crate) mod warper;

pub use lasso::{Lasso, PolyLineLasso};
pub use load::{FileLoader, FutureLoader, RawBytes};
pub use popup::PopupMsg;
pub use url::URLManager;

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
