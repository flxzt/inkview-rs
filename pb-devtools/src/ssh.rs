use std::path::PathBuf;

use crate::DevtoolsError;

pub async fn start_server(config_dir: PathBuf, port: u16) -> Result<(), DevtoolsError> {
    let mut ssh_server = redlight::SshServer::init(config_dir.join("redlight")).await?;
    ssh_server
        .run(("0.0.0.0", port))
        .await
        .map_err(DevtoolsError::from)
}
