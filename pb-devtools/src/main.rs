use std::path::PathBuf;

use clap::Parser;
use inkview::bindings;
use pb_devtools::{appreceive, debug, ssh, status, wifi, DevtoolsError};
use tracing::Instrument;

#[derive(Debug, clap::Parser)]
#[command(version, about, long_about = None)]
pub(crate) struct Args {
    #[command(subcommand)]
    action: Action,
}

#[derive(Debug, clap::Subcommand)]
pub(crate) enum Action {
    ReportStatus,
    Wifi {
        #[command(subcommand)]
        action: WifiAction,
    },
    Ssh {
        #[arg(short, long, default_value = "./pb-devtools-data")]
        config_path: PathBuf,
        #[arg(short, long, default_value = "2345")]
        port: u16,
    },
    DebugSession {
        #[arg(short, long, default_value = "10003")]
        port: u16,
        #[arg(short, long)]
        executable: PathBuf,
    },
    AppReceive {
        #[arg(short, long, default_value = "19991")]
        port: u16,
        /// The target file of the received binary
        target_file: PathBuf,
    },
}

#[derive(Debug, clap::Subcommand)]
pub(crate) enum WifiAction {
    Activate,
    Deactivate,
}

fn main() {
    let iv = Box::leak(Box::new(inkview::load())) as &_;
    let cancel_token = tokio_util::sync::CancellationToken::new();
    let canceller = cancel_token.clone();
    let (run_tx, mut run_rx) = tokio::sync::mpsc::unbounded_channel::<()>();

    if let Err(e) = setup_logging() {
        eprintln!("Failed to setup logging, Err: {e:?}");
    }

    let rt_thread = std::thread::spawn(move || loop {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create async runtime.");

        rt.block_on(async {
            loop {
                tokio::select! {
                    _ = run_rx.recv() => {
                        let canceller = cancel_token.clone();
                        tokio::spawn(async move {
                            let args = Args::parse();
                            if let Err(e) = run(iv, args).await {
                                tracing::error!("Running devtools failed, Err: {e:?}");
                            }
                            canceller.cancel();
                        }.instrument(tracing::span!(tracing::Level::INFO, "Run task")));
                    }
                    _ = cancel_token.cancelled() => {
                        unsafe {
                           iv.CloseApp();
                        }
                        break;
                    }
                }
            }
        });
    });

    inkview::iv_main(iv, move |event| {
        match event {
            inkview::Event::Init => {
                // Inkview's init is verbose, this separates the Cli output.
                println!("\n\n\n");

                if run_tx.send(()).is_err() {
                    eprintln!("Running devtools action failed.");
                    unsafe {
                        iv.CloseApp();
                    }
                }
            }
            inkview::Event::Exit => {
                canceller.cancel();
            }
            _ => {}
        }
        Some(())
    });

    rt_thread
        .join()
        .expect("Async runtime thread exited with error.");
}

fn setup_logging() -> Result<(), tracing::dispatcher::SetGlobalDefaultError> {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_ansi(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    tracing::debug!("tracing initialized.. ");
    Ok(())
}

async fn run(iv: &'static bindings::Inkview, args: Args) -> Result<(), DevtoolsError> {
    match args.action {
        Action::ReportStatus => {
            let status = status::report_status(iv)?;
            println!("{status}");
        }
        Action::Wifi { action } => match action {
            WifiAction::Activate => wifi::wifi_activate(iv, false)?,
            WifiAction::Deactivate => wifi::wifi_deactivate(iv)?,
        },
        Action::Ssh { config_path, port } => ssh::start_server(config_path, port).await?,
        Action::DebugSession { executable, port } => {
            wifi::wifi_activate(iv, false)?;
            let mut session = debug::DebugSession::start(executable, port).await?;
            session.wait().await?;
        }
        Action::AppReceive { port, target_file } => {
            appreceive::wait_receive_write(port, target_file).await?;
        }
    }
    Ok(())
}
