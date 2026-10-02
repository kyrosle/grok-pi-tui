/// A native update notification shared by the stock and Pi update producers.
#[derive(Debug, Clone)]
pub struct UpdateAvailable {
    /// The latest version string (e.g. "0.1.200").
    pub latest_version: String,
}
