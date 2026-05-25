use std::{net::SocketAddr, path::PathBuf, process::ExitCode, sync::Arc};

use clap::Parser;
use deepseek_mobile_web_server::{
    AppState, MobileWebServerConfig, SshTarget,
    agent_model::{AgentModel, DeepSeekAgentModel, MockAgentModel},
    app_router_with_config_access_token_and_model, app_router_with_config_and_model,
    model_config::{MobileModelConfig, MobileModelMode},
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
    #[arg(long, value_parser = ["auto", "mock", "deepseek"])]
    model_mode: Option<String>,
    #[arg(long)]
    model_config: Option<PathBuf>,
    #[arg(long)]
    static_dir: Option<PathBuf>,
    #[arg(long)]
    access_token: Option<String>,
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
    let model_mode = resolve_model_mode(&args)?;
    let model_config = load_model_config(model_mode, args.model_config.as_ref())?;
    let model_status = model_config.redacted_status();
    let model = build_model(model_config);
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
        "model mode {} model {} api_key_present {}",
        model_status.provider, model_status.model, model_status.api_key_present
    );
    info!(
        "access token {}",
        if args.access_token.is_some() {
            "enabled"
        } else {
            "disabled"
        }
    );

    let static_dir = args.static_dir;
    if let Some(static_dir) = &static_dir {
        info!("static dir {}", static_dir.display());
    }

    let listener = TcpListener::bind(bind_addr).await?;
    let state = AppState::new(target);
    let config = MobileWebServerConfig {
        use_real_model: args.use_real_model,
        static_dir,
    };
    let app = if let Some(access_token) = args.access_token {
        app_router_with_config_access_token_and_model(state, config, access_token, model)
    } else {
        app_router_with_config_and_model(state, config, model)
    };
    axum::serve(listener, app).await?;
    Ok(())
}

fn resolve_model_mode(args: &Args) -> Result<MobileModelMode, Box<dyn std::error::Error>> {
    if args.use_real_model {
        return Ok(MobileModelMode::Deepseek);
    }
    if let Some(mode) = &args.model_mode {
        return Ok(mode.parse()?);
    }
    if let Ok(mode) = std::env::var("DEEPSEEK_MOBILE_MODEL_MODE")
        && !mode.trim().is_empty()
    {
        return Ok(mode.parse()?);
    }
    Ok(MobileModelMode::Auto)
}

fn load_model_config(
    mode: MobileModelMode,
    path: Option<&PathBuf>,
) -> Result<MobileModelConfig, Box<dyn std::error::Error>> {
    if let Some(path) = path {
        Ok(MobileModelConfig::load_from_path(path, mode)?)
    } else {
        Ok(MobileModelConfig::load_default(mode)?)
    }
}

fn build_model(config: MobileModelConfig) -> Arc<dyn AgentModel> {
    if config.provider == "mock" {
        Arc::new(MockAgentModel::new())
    } else {
        Arc::new(DeepSeekAgentModel::new(config))
    }
}
