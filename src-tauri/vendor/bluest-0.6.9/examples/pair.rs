use std::error::Error;

use async_trait::async_trait;
use bluest::pairing::{IoCapability, PairingAgent, PairingRejected, Passkey};
use bluest::{btuuid, Adapter, Device};
use futures_lite::StreamExt;
use tracing::info;
use tracing::metadata::LevelFilter;

struct StdioPairingAgent;

#[async_trait]
impl PairingAgent for StdioPairingAgent {
    /// The input/output capabilities of this agent
    fn io_capability(&self) -> IoCapability {
        IoCapability::KeyboardDisplay
    }

    async fn confirm(&self, device: &Device) -> Result<(), PairingRejected> {
        tokio::task::block_in_place(move || {
            println!("Do you want to pair with {:?}? (Y/n)", device.name().unwrap());
            let mut buf = String::new();
            std::io::stdin()
                .read_line(&mut buf)
                .map_err(|_| PairingRejected::default())?;
            let response = buf.trim();
            if response.is_empty() || response == "y" || response == "Y" {
                Ok(())
            } else {
                Err(PairingRejected::default())
            }
        })
    }

    async fn confirm_passkey(&self, device: &Device, passkey: Passkey) -> Result<(), PairingRejected> {
        tokio::task::block_in_place(move || {
            println!(
                "Is the passkey \"{}\" displayed on {:?}? (Y/n)",
                passkey,
                device.name().unwrap()
            );
            let mut buf = String::new();
            std::io::stdin()
                .read_line(&mut buf)
                .map_err(|_| PairingRejected::default())?;
            let response = buf.trim();
            if response.is_empty() || response == "y" || response == "Y" {
                Ok(())
            } else {
                Err(PairingRejected::default())
            }
        })
    }

    async fn request_passkey(&self, device: &Device) -> Result<Passkey, PairingRejected> {
        tokio::task::block_in_place(move || {
            println!("Please enter the 6-digit passkey for {:?}: ", device.name().unwrap());
            let mut buf = String::new();
            std::io::stdin()
                .read_line(&mut buf)
                .map_err(|_| PairingRejected::default())?;
            buf.trim().parse().map_err(|_| PairingRejected::default())
        })
    }

    fn display_passkey(&self, device: &Device, passkey: Passkey) {
        println!("The passkey is \"{}\" for {:?}.", passkey, device.name().unwrap());
    }
}

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
        let mut scan = adapter.scan(&[btuuid::services::HUMAN_INTERFACE_DEVICE]).await?;
        info!("scan started");
        scan.next().await.ok_or("scan terminated")?
    };

    info!("{:?} {:?}", discovered_device.rssi, discovered_device.adv_data);
    let device = discovered_device.device;

    adapter.connect_device(&device).await?;
    info!("connected!");

    device.pair_with_agent(&StdioPairingAgent).await?;
    info!("paired!");

    adapter.disconnect_device(&device).await?;
    info!("disconnected!");

    Ok(())
}
