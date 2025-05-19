pub(crate) mod daemon;
pub(crate) mod display;
pub(crate) mod error;
pub(crate) mod grpc;

pub(crate) use daemon::Daemon;

#[tokio::main]
async fn main() -> Result<(), i32> {
    let iv = Box::leak(Box::new(inkview::load())) as &_;

    let _daemon = match Daemon::run(iv) {
        Ok(daemon) => daemon,
        Err(e) => {
            tracing::error!("Running daemon failed, Err: {e:?}");
            return Err(1);
        }
    };
    Ok(())
}
