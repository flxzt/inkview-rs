use crate::display::DisplayState;
use crate::error::DaemonError;
use crate::grpc;
use inkview::bindings::Inkview;
use inkview::event::Key;
use inkview::Event;
use inkview_eg::InkviewDisplay;
use lockfile::Lockfile;
use std::cell::OnceCell;
use std::path::PathBuf;
use std::time::Duration;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tracing_subscriber::fmt::writer::MakeWriterExt;

#[allow(unused)]
pub(crate) const DAEMON_NAME: &str = "pb-devtools-daemon";
const WIFI_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);
const DEVTOOLS_DAEMON_DATA_DIR: &str = "./pb-devtools-daemon-data";
const SSH_SERVER_PORT: u16 = 2345;

const RUNNING_LOCKFILE: &str = "/tmp/pb-devtools-daemon.lock";

#[derive(Debug)]
pub enum Msg {
    InkviewEvent(Event),
    WifiKeepAlive,
    PrintStatus,
    Quit,
}

impl std::fmt::Display for Msg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Msg::InkviewEvent(event) => write!(f, "InkviewEvent '{event:?}'"),
            Msg::WifiKeepAlive => write!(f, "WifiKeepAlive"),
            Msg::PrintStatus => write!(f, "PrintStatus"),
            Msg::Quit => write!(f, "Quit"),
        }
    }
}

pub(crate) struct Daemon {
    _running_lockfile: Lockfile,
    _file_log_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}

impl Daemon {
    pub(crate) fn run(iv: &'static Inkview) -> Result<Self, DaemonError> {
        let running_lockfile = match Lockfile::create(RUNNING_LOCKFILE) {
            Ok(l) => Ok(l),
            Err(lockfile::Error::LockTaken) => Err(DaemonError::DaemonAlreadyRunning),
            Err(lockfile::Error::Io(source)) => Err(DaemonError::Io { source }),
            Err(_) => Err(DaemonError::Other(
                "Other Lockfile Error happeneded".to_string(),
            )),
        }?;

        let (msg_tx, msg_rx) = mpsc::unbounded_channel::<Msg>();
        let file_log_guard = match setup_logging() {
            Ok(g) => Some(g),
            Err(e) => {
                eprintln!("Failed to setup logging, Err: {e:?}");
                None
            }
        };
        tracing::debug!("Starting daemon.");

        spawn_message_handler_task(iv, msg_tx.clone(), msg_rx);

        inkview::iv_main(iv, move |event| {
            if msg_tx.clone().send(Msg::InkviewEvent(event)).is_err() {
                tracing::error!("Failed to send InkviewEvent message, receiver closed.");
                unsafe {
                    iv.CloseApp();
                }
            }
            Some(())
        });

        Ok(Self {
            _running_lockfile: running_lockfile,
            _file_log_guard: file_log_guard,
        })
    }
}

/// Returns a guard for log file writing that flushes any remaining logs when dropped.
fn setup_logging() -> Result<tracing_appender::non_blocking::WorkerGuard, DaemonError> {
    let log_file_dir = PathBuf::from(DEVTOOLS_DAEMON_DATA_DIR);
    let log_file_name = PathBuf::from("log");
    let log_file_path = log_file_dir.join(&log_file_name);

    std::fs::File::create(log_file_path).map_err(|_| DaemonError::Logging)?;

    let appender = tracing_appender::rolling::never(log_file_dir, log_file_name);
    let (file_appender, guard) = tracing_appender::non_blocking(appender);

    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stdout.and(file_appender))
        .with_ansi(false)
        .finish();

    tracing::subscriber::set_global_default(subscriber).map_err(|_| DaemonError::Logging)?;
    tracing::debug!("tracing initialized.. ");

    Ok(guard)
}

fn spawn_message_handler_task(
    iv: &'static Inkview,
    msg_tx: UnboundedSender<Msg>,
    mut msg_rx: UnboundedReceiver<Msg>,
) {
    tokio::task::spawn_blocking(move || {
        let mut display_state = DisplayState::default();
        let mut display: OnceCell<InkviewDisplay> = OnceCell::new();

        while let Some(msg) = msg_rx.blocking_recv() {
            tracing::info!("Handling received message: '{msg}'");

            match msg {
                Msg::InkviewEvent(event) => match handle_inkview_event(
                    iv,
                    &mut display_state,
                    &mut display,
                    msg_tx.clone(),
                    event,
                ) {
                    Ok(quit) => {
                        if quit {
                            break;
                        }
                    }
                    Err(e) => tracing::error!("Failed handling inkview event, Err: {e:}"),
                },
                Msg::WifiKeepAlive => {
                    if let Err(e) = pb_devtools::wifi::wifi_keepalive(iv) {
                        tracing::error!("Failed to keep alive wifi, Err: {e:?}");
                    }
                }
                Msg::PrintStatus => match pb_devtools::status::report_status(iv) {
                    Ok(s) => println!("{s}"),
                    Err(e) => tracing::error!("Failed to get status report, Err: {e:?}"),
                },
                Msg::Quit => break,
            }
        }

        tracing::info!("Exiting daemon.");

        unsafe {
            iv.CloseApp();
        }
    });
}

/// Handle an incoming inkview event.
///
/// Returns Ok(true) when the app should be quit.
fn handle_inkview_event(
    iv: &'static Inkview,
    display_state: &mut DisplayState,
    display: &mut OnceCell<InkviewDisplay>,
    msg_tx: UnboundedSender<Msg>,
    event: inkview::Event,
) -> Result<bool, DaemonError> {
    let mut repaint = false;

    match event {
        Event::Init => {
            if display.set(InkviewDisplay::new(iv)).is_err() {
                tracing::error!(
                    "Inkview display was already initialized when inkview init event was received."
                );
            }
            daemon_init(iv, msg_tx.clone())?;
            repaint = true;
        }
        Event::Show | Event::Repaint => {
            repaint = true;
        }
        Event::KeyDown { key } => match key {
            Key::Prev | Key::Left => {
                display_state.page_prev();
                repaint = true;
            }
            Key::Next | Key::Right => {
                display_state.page_next();
                repaint = true;
            }
            _ => {}
        },
        Event::Exit => return Ok(true),
        _ => {}
    }

    if repaint {
        let Some(display) = display.get_mut() else {
            tracing::warn!("Display not initialized yet when trying to repaint.");
            return Ok(false);
        };
        display_state.paint(iv, display)?;
    }

    Ok(false)
}

fn daemon_init(iv: &'static Inkview, msg_tx: UnboundedSender<Msg>) -> Result<(), DaemonError> {
    pb_devtools::wifi::wifi_activate(iv, true)?;
    spawn_grpc_server_task(iv, msg_tx.clone());
    spawn_wifi_keepalive_task(msg_tx.clone());
    spawn_ssh_server_task(msg_tx.clone());
    spawn_sig_handler_task(msg_tx.clone());
    Ok(())
}

fn spawn_sig_handler_task(msg_tx: UnboundedSender<Msg>) {
    tokio::spawn(async move {
        let res = async move {
            let mut sig_term = signal(SignalKind::terminate()).map_err(|source| {
                DaemonError::UnixSignalHandler {
                    sig: SignalKind::terminate(),
                    source,
                }
            })?;
            let mut sig_int = signal(SignalKind::interrupt()).map_err(|source| {
                DaemonError::UnixSignalHandler {
                    sig: SignalKind::interrupt(),
                    source,
                }
            })?;

            loop {
                tokio::select! {
                    s = sig_term.recv() => {
                        if s.is_some() {
                        break;
                        }
                    }
                    s = sig_int.recv() => {
                        if s.is_some() {
                        break;
                        }
                    }
                }
            }

            msg_tx.send(Msg::Quit).map_err(|_| DaemonError::MessageSend)
        };

        if let Err(e) = res.await {
            tracing::error!("Signal handler task terminated with Err: {e:?}");
        }
    });
}

fn spawn_wifi_keepalive_task(msg_tx: UnboundedSender<Msg>) {
    tokio::spawn(async move {
        let res = async move {
            loop {
                if msg_tx.send(Msg::WifiKeepAlive).is_err() {
                    return Err(DaemonError::MessageSend) as Result<(), DaemonError>;
                }
                tokio::time::sleep(WIFI_KEEPALIVE_INTERVAL).await;
            }
        };

        if let Err(e) = res.await {
            tracing::error!("Wifi keepalive task terminated with Err: {e:?}");
        }
    });
}

fn spawn_grpc_server_task(iv: &'static Inkview, msg_tx: UnboundedSender<Msg>) {
    tokio::spawn(async move {
        let res = async move {
            grpc::start_server(iv, msg_tx.clone()).await?;
            msg_tx.send(Msg::Quit).map_err(|_| DaemonError::MessageSend)
        };

        if let Err(e) = res.await {
            tracing::error!("Grpc server task terminated with Err: {e:?}");
        }
    });
}

fn spawn_ssh_server_task(_msg_tx: UnboundedSender<Msg>) {
    tokio::spawn(async move {
        pb_devtools::ssh::start_server(PathBuf::from(DEVTOOLS_DAEMON_DATA_DIR), SSH_SERVER_PORT)
            .await
    });
}
