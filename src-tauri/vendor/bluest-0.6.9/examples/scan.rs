use std::error::Error;

use bluest::Adapter;
use futures_lite::StreamExt;
use tracing::info;
use tracing::metadata::LevelFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{fmt, EnvFilter};

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let adapter = Adapter::default().await.ok_or("Bluetooth adapter not found")?;
    adapter.wait_available().await?;

    info!("starting scan");
    let mut scan = adapter.scan(&[]).await?;
    info!("scan started");
    while let Some(discovered_device) = scan.next().await {
        info!(
            "{}{}: {:?}",
            discovered_device.device.name().as_deref().unwrap_or("(unknown)"),
            discovered_device
                .rssi
                .map(|x| format!(" ({}dBm)", x))
                .unwrap_or_default(),
            discovered_device.adv_data.services
        );
    }

    Ok(())
}
