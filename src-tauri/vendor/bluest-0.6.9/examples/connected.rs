use std::error::Error;

use bluest::Adapter;
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

    info!("getting connected devices");
    let devices = adapter.connected_devices().await?;
    for device in devices {
        info!("found {:?}", device);
        adapter.connect_device(&device).await?;
        let services = device.services().await?;
        for service in services {
            info!("  {:?}", service);
            let characteristics = service.characteristics().await?;
            for characteristic in characteristics {
                info!("    {:?}", characteristic);
                let props = characteristic.properties().await?;
                info!("      props: {:?}", props);
                if props.read {
                    info!("      value: {:?}", characteristic.read().await);
                }
                if props.write_without_response {
                    info!("      max_write_len: {:?}", characteristic.max_write_len());
                }

                let descriptors = characteristic.descriptors().await?;
                for descriptor in descriptors {
                    info!("      {:?}: {:?}", descriptor, descriptor.read().await);
                }
            }
        }
    }
    info!("done");

    Ok(())
}
