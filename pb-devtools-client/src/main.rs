use std::process;

pub(crate) mod app;

#[tokio::main]
async fn main() {
    if let Err(e) = app::run().await {
        eprintln!("App reported Err: {e:?}");
        process::exit(1);
    }
}
