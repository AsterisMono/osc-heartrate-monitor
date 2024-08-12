use btleplug::api::{bleuuid::uuid_from_u16, Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager, Peripheral};
use std::error::Error;
use std::time::Duration;
use tokio::time;
use tokio_stream::StreamExt;
use uuid::Uuid;

const HR_SERVICE_UUID: Uuid = uuid_from_u16(0x180D);
const HR_CHARACTERISTIC_UUID: Uuid = uuid_from_u16(0x2A37);

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let manager = Manager::new().await.unwrap();

    let adapters = manager.adapters().await?;
    let adapter = adapters.first().ok_or("No adapter found")?;

    loop {
        // start scanning for devices
        adapter.start_scan(ScanFilter::default()).await?;
        time::sleep(Duration::from_secs(2)).await;

        // Find devices with heart rate service
        let hr_device = find_hr_device(&adapter).await;

        match hr_device {
            Some(device) => {
                println!(
                    "Found heart rate device: {:?}, connecting",
                    device.properties().await?.unwrap().local_name
                );
                // Connect to heart rate device
                device.connect().await?;

                // Subscribe to heart rate notifications
                let characteristics = device.characteristics();
                let hr_char = characteristics
                    .iter()
                    .find(|c| c.uuid == HR_CHARACTERISTIC_UUID)
                    .unwrap();

                device.subscribe(hr_char).await?;
                println!(
                    "Connected to heart rate device: {:?}",
                    device.properties().await?.unwrap().local_name
                );

                // Handle heart rate notifications
                let mut notification_stream = device.notifications().await?;
                while let Some(data) = notification_stream.next().await {
                    if data.uuid == HR_CHARACTERISTIC_UUID {
                        let hr_value = data.value[1];
                        println!("Heart rate: {} BPM", hr_value);
                    }
                }
                break; // Successfully connected and handling data, exit loop
            }
            None => {
                println!("Heart rate device not found, retrying...");
                // Continue loop, rescan
            }
        }
    }

    Ok(())
}

async fn find_hr_device(adapter: &Adapter) -> Option<Peripheral> {
    for p in adapter.peripherals().await.unwrap() {
        if let Some(properties) = p.properties().await.unwrap() {
            if properties.services.contains(&HR_SERVICE_UUID) {
                return Some(p);
            }
        }
    }
    None
}
