use tokio::signal::unix::SignalKind;

#[derive(Debug, thiserror::Error)]
pub(crate) enum DaemonError {
    #[error("Daemon is already running")]
    DaemonAlreadyRunning,
    #[error("UNIX Signal handler setup failed")]
    UnixSignalHandler {
        sig: SignalKind,
        source: std::io::Error,
    },
    #[error("Setup logging failed")]
    Logging,
    #[error("Sending message failed")]
    MessageSend,
    #[error("Error with Grpc server")]
    GrpcServer { source: tonic::transport::Error },
    #[error("Devtools error occurred")]
    Devtools(#[from] pb_devtools::DevtoolsError),
    #[error("Inkview error occurred")]
    Inkview { code: Option<i32>, msg: String },
    #[error("Generic Io Error")]
    Io { source: std::io::Error },
    #[error("Other error occurred: '{0:?}'")]
    Other(String),
}
