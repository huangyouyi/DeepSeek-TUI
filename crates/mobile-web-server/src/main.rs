use std::{net::SocketAddr, path::PathBuf, process::ExitCode};

use clap::Parser;
use deepseek_mobile_web_server::{
    AppState, MobileWebServerConfig, SshTarget, app_router_with_config,
};
use tokio::net::TcpListener;
use tracing::info;

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    #[arg(long, default_value = "0.0.0.0")]
    host: String,
    #[arg(long, default_value_t = 8788)]
    port: u16,
    #[arg(long, default_value = "192.168.30.244")]
    ssh_host: String,
    #[arg(long, default_value = "root")]
    ssh_user: String,
    #[arg(long, default_value_t = 22)]
    ssh_port: u16,
    #[arg(long, default_value_t = false)]
    use_real_model: bool,
    #[arg(long)]
    static_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    match run(Args::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("mobile web server failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let bind_addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    let target = SshTarget {
        host: args.ssh_host,
        user: args.ssh_user,
        port: args.ssh_port,
        key_present: false,
    };

    info!("listening on http://{bind_addr}");
    info!("lan url http://{}:{}", args.host, args.port);
    info!("ssh target {}@{}:{}", target.user, target.host, target.port);
    info!(
        "model access {}",
        if args.use_real_model { "real" } else { "mock" }
    );

    let static_dir = args.static_dir;
    if let Some(static_dir) = &static_dir {
        info!("static dir {}", static_dir.display());
    }

    let listener = TcpListener::bind(bind_addr).await?;
    let app = app_router_with_config(
        AppState::new(target),
        MobileWebServerConfig {
            use_real_model: args.use_real_model,
            static_dir,
        },
    );
    axum::serve(listener, app).await?;
    Ok(())
}
