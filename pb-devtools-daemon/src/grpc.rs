use inkview::bindings::Inkview;
use std::net::SocketAddr;
use tokio::sync::mpsc::UnboundedSender;

use crate::daemon::Msg;
use crate::error::DaemonError;

pub(crate) const SERVER_ADDR: &str = "0.0.0.0:50051";

mod daemon_proto {
    tonic::include_proto!("daemon"); // The string specified here must match the proto package name
}

struct DaemonProto {
    iv: &'static Inkview,
    msg_tx: UnboundedSender<Msg>,
}

impl DaemonProto {
    fn new(iv: &'static Inkview, msg_tx: UnboundedSender<Msg>) -> Self {
        Self { iv, msg_tx }
    }
}

#[tonic::async_trait]
impl daemon_proto::daemon_server::Daemon for DaemonProto {
    async fn print_status(
        &self,
        _request: tonic::Request<daemon_proto::PrintStatusRequest>,
    ) -> Result<tonic::Response<daemon_proto::PrintStatusReply>, tonic::Status> {
        if let Err(e) = self.msg_tx.send(Msg::PrintStatus) {
            tracing::error!("Failed to send PrintStatus message, Err: {e:?}");
        }
        Ok(tonic::Response::new(daemon_proto::PrintStatusReply {}))
    }

    async fn report_status(
        &self,
        _request: tonic::Request<daemon_proto::ReportStatusRequest>,
    ) -> Result<tonic::Response<daemon_proto::ReportStatusReply>, tonic::Status> {
        let status = match pb_devtools::status::report_status(self.iv) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Getting status for report failed, Err: {e:?}");
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    "Getting status for report failed",
                ));
            }
        };

        Ok(tonic::Response::new(daemon_proto::ReportStatusReply {
            status,
        }))
    }
}

pub(crate) async fn start_server(
    iv: &'static Inkview,
    msg_tx: UnboundedSender<Msg>,
) -> Result<(), DaemonError> {
    let addr: SocketAddr = SERVER_ADDR.parse().unwrap();
    let daemon_proto = DaemonProto::new(iv, msg_tx);

    tonic::transport::Server::builder()
        .add_service(daemon_proto::daemon_server::DaemonServer::new(daemon_proto))
        .serve(addr)
        .await
        .map_err(|source| DaemonError::GrpcServer { source })
}
