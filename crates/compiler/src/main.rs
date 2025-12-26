use anyhow::Result;
use fusion::cli::exec_cli;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    exec_cli()
}
