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

    let adapter = Adapter::default().await.ok_or("Bluetooth adapter not found")?;
    adapter.wait_available().await?;

    let discovered_device = {
        info!("starting scan");
        let services = &[btuuid::services::USER_DATA];
        let mut scan = adapter.scan(services).await?;
        info!("scan started");
        scan.next().await.ok_or("scan terminated")?
    };

    info!("{:?} {:?}", discovered_device.rssi, discovered_device.adv_data);
    adapter.connect_device(&discovered_device.device).await?;
    info!("connected!");

    tokio::time::sleep(Duration::from_secs(30)).await;

    adapter.disconnect_device(&discovered_device.device).await?;
    info!("disconnected!");

    Ok(())
}
