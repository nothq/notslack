#[derive(Clone, Debug)]
pub struct RemoteImageData {
    pub bytes: Vec<u8>,
    pub mimetype: String,
}

pub trait RemoteImageApi: Send + Sync {
    fn load_remote_image(&self, url: &str) -> Result<Option<RemoteImageData>, String>;
}

impl<F> RemoteImageApi for F
where
    F: Fn(&str) -> Result<Option<RemoteImageData>, String> + Send + Sync,
{
    fn load_remote_image(&self, url: &str) -> Result<Option<RemoteImageData>, String> {
        self(url)
    }
}
