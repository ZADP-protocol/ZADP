//! Standalone canonical discovery: no hosted moderation dependency.
use clap::Parser;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    zadp_api::runtime::run(
        zadp_api::runtime::Args::parse(),
        |registry, versions, metadata| {
            Ok(zadp_api::multi_board_router_with_metadata(
                registry, versions, metadata,
            ))
        },
    )
    .await
}
