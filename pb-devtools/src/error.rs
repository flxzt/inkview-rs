#[derive(Debug, thiserror::Error)]
pub enum DevtoolsError {
    #[error("Setup logging failed")]
    Logging,
    #[error("Received invalid command")]
    InvalidCommand,
    #[error("Connecting to Wifi failed")]
    WifiConnect,
    #[error("Disconnecting to Wifi failed")]
    WifiDisconnect,
    #[error("Ssh Server failed")]
    Ssh(#[from] redlight::RedlightError),
    #[error("App receiver failed.")]
    AppReceiveIo { source: std::io::Error },
    #[error("Debug-Session Io failed.")]
    DebugSessionIo { msg: String, source: std::io::Error },
    #[error("Inkview error occurred.")]
    Inkview { code: Option<i32>, msg: String },
    #[error("Other error occurred: '{0:?}'")]
    Other(String),
}
