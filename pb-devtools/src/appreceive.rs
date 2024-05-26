use std::path::PathBuf;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::DevtoolsError;

/// Open a TCP socket with the provided port, wait and receive data and save this data as a executable file.
pub async fn wait_receive_write(port: u16, target_file_path: PathBuf) -> Result<(), DevtoolsError> {
    let listen_address: (&str, u16) = ("0.0.0.0", port);
    let listener = TcpListener::bind(listen_address)
        .await
        .map_err(|source| DevtoolsError::AppReceiveIo { source })?;

    eprintln!("Wait on TCP socket with address: '{listen_address:?}'");
    let (mut stream, addr) = listener
        .accept()
        .await
        .map_err(|source| DevtoolsError::AppReceiveIo { source })?;

    eprintln!("Receiving data from address: '{addr:?}'");
    let mut data = Vec::<u8>::new();
    stream
        .read_to_end(&mut data)
        .await
        .map_err(|source| DevtoolsError::AppReceiveIo { source })?;
    stream
        .shutdown()
        .await
        .map_err(|source| DevtoolsError::AppReceiveIo { source })?;

    eprintln!(
        "Write received data to target file: '{}'",
        target_file_path.display()
    );
    let mut file = tokio::fs::File::create(&target_file_path)
        .await
        .map_err(|source| DevtoolsError::AppReceiveIo { source })?;
    file.write_all(&data)
        .await
        .map_err(|source| DevtoolsError::AppReceiveIo { source })?;
    file.flush()
        .await
        .map_err(|source| DevtoolsError::AppReceiveIo { source })
}
