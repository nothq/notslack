pub type CacheKeyRead = dyn Fn(String) -> Result<Option<String>, String> + Send + Sync;
pub type CacheKeyWrite = dyn Fn(String, String) -> Result<(), String> + Send + Sync;

pub struct CacheKeyTestOverride {
    pub read: Box<CacheKeyRead>,
    pub write: Box<CacheKeyWrite>,
}

pub type CacheKeyOverrideState = std::sync::Mutex<Option<CacheKeyTestOverride>>;

pub fn cache_key_test_override() -> &'static CacheKeyOverrideState {
    static OVERRIDE: std::sync::OnceLock<CacheKeyOverrideState> = std::sync::OnceLock::new();
    OVERRIDE.get_or_init(|| std::sync::Mutex::new(None))
}

pub fn set_test_cache_key_store<R, W>(read: R, write: W) -> TestCacheKeyStoreGuard
where
    R: Fn(String) -> Result<Option<String>, String> + Send + Sync + 'static,
    W: Fn(String, String) -> Result<(), String> + Send + Sync + 'static,
{
    *cache_key_test_override()
        .lock()
        .expect("cache key override lock") = Some(CacheKeyTestOverride {
        read: Box::new(read),
        write: Box::new(write),
    });
    TestCacheKeyStoreGuard
}

pub struct TestCacheKeyStoreGuard;

impl Drop for TestCacheKeyStoreGuard {
    fn drop(&mut self) {
        cache_key_test_override()
            .lock()
            .expect("cache key override lock")
            .take();
    }
}
