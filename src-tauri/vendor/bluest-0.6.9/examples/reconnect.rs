use std::error::Error;
use std::time::Duration;

use bluest::{btuuid, Adapter};
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

    let device_id = {
        let adapter = Adapter::default().await.unwrap();
        adapter.wait_available().await?;

        info!("looking for device");
        let device = adapter
            .discover_devices(&[btuuid::services::BATTERY])
            .await?
            .next()
            .await
            .ok_or("Failed to discover device")??;
        info!(
            "found device: {} ({:?})",
            device.name().as_deref().unwrap_or("(unknown)"),
            device.id()
        );

        device.id()
    };

    info!("Time passes...");
    tokio::time::sleep(Duration::from_secs(5)).await;

    {
        let adapter = Adapter::default().await.unwrap();
        adapter.wait_available().await?;

        info!("re-opening previously found device");
        let device = adapter.open_device(&device_id).await?;
        info!(
            "re-opened device: {} ({:?})",
            device.name().as_deref().unwrap_or("(unknown)"),
            device.id()
        );
    }

    Ok(())
}
