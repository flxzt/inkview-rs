use std::net::SocketAddr;

mod daemon_proto {
    tonic::include_proto!("daemon"); // The string specified here must match the proto package name
}

const HELP: &str = "\
pb-devtools-client

USAGE:
    pb-devtools-client <COMMAND>

ARGS:
    <COMMAND>:
        report-status       Report the device status to the client
        print-status        Print the status on the device
";

pub(crate) struct Args {
    server: SocketAddr,
    command: String,
}

impl Args {
    fn parse() -> anyhow::Result<Self> {
        let mut pargs = pico_args::Arguments::from_env();
        if pargs.contains(["-h", "--help"]) {
            print!("{}", HELP);
            std::process::exit(0);
        }
        let args = Self {
            server: pargs.value_from_str::<&str, String>("--server")?.parse()?,
            command: pargs.free_from_str::<String>()?,
        };
        Ok(args)
    }
}

pub(crate) async fn run() -> anyhow::Result<()> {
    let args = Args::parse()?;
    let server_addr = args.server;
    let mut client =
        daemon_proto::daemon_client::DaemonClient::connect(format!("http://{server_addr}")).await?;

    match args.command.as_str() {
        "report-status" => {
            let request = tonic::Request::new(daemon_proto::ReportStatusRequest {});
            let response = client.report_status(request).await?;
            println!("{}", response.into_inner().status);
        }
        "print-status" => {
            let request = tonic::Request::new(daemon_proto::PrintStatusRequest {});
            let _response = client.print_status(request).await?;
            println!("Status printed!");
        }
        _ => return Err(anyhow::anyhow!("Invalid command.")),
    }

    Ok(())
}
