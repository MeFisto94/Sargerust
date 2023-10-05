pub trait AssetLoader<T> {
    fn load(&self, path: &str) -> T;
}

pub trait RawAssetLoader {
    fn load_raw(&self, path: &str) -> &[u8];

    /// in case of a caching implementation, this may need to clone the whole buffer!
    fn load_raw_owned(&self, path: &str) -> Option<Vec<u8>>; // TODO: Result!
}