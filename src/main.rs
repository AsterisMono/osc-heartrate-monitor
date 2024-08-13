use btleplug::api::{bleuuid::uuid_from_u16, Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager, Peripheral};
use rosc::{OscPacket, OscType};
use std::error::Error;
use std::net::UdpSocket;
use std::time::Duration;
use tokio::time;
use tokio_stream::StreamExt;
use uuid::Uuid;

const HR_SERVICE_UUID: Uuid = uuid_from_u16(0x180D);
const HR_CHARACTERISTIC_UUID: Uuid = uuid_from_u16(0x2A37);
const OSC_PARAM_HR_CONNECTED: &str = "/avatar/parameters/hr_connected";
const OSC_PARAM_HR_PERCENT: &str = "/avatar/parameters/hr_percent";
const MAX_HEART_RATE: i32 = 200;
const OSC_ADDRESS: &str = "127.0.0.1:9000"; // OSC server address, modify as needed

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let manager = Manager::new().await.unwrap();

    let adapters = manager.adapters().await?;
    let adapter = adapters.first().ok_or("No adapter found")?;

    let osc_socket = UdpSocket::bind("0.0.0.0:0")?;

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
                    device.properties().await?.unwrap().local_name.unwrap()
                );

                // Send connection status as true
                send_osc_message(&osc_socket, OSC_PARAM_HR_CONNECTED, OscType::Bool(true))?;

                match connect_and_handle_device(&device, &osc_socket).await {
                    Ok(_) => {
                        println!("Device connection closed, retrying...");
                    }
                    Err(e) => {
                        println!("Error connecting device: {:?}, retrying...", e);
                    }
                }

                // Send connection status as false
                send_osc_message(&osc_socket, OSC_PARAM_HR_CONNECTED, OscType::Bool(false))?;
            }
            None => {
                println!("Heart rate device not found, retrying...");
            }
        }

        // Wait for some time before retrying
        time::sleep(Duration::from_secs(2)).await;
    }
}

async fn connect_and_handle_device(
    device: &Peripheral,
    osc_socket: &UdpSocket,
) -> Result<(), Box<dyn Error>> {
    device.connect().await?;
    device.discover_services().await?;

    let characteristics = device.characteristics();
    let hr_char = characteristics
        .iter()
        .find(|c| c.uuid == HR_CHARACTERISTIC_UUID)
        .ok_or("Heart rate characteristic not found")?;

    device.subscribe(hr_char).await?;
    println!(
        "Connected to heart rate device: {:?}",
        device.properties().await?.unwrap().local_name.unwrap()
    );

    let mut notification_stream = device.notifications().await?;

    while let Some(data) = notification_stream.next().await {
        if data.uuid == HR_CHARACTERISTIC_UUID {
            let hr_value = data.value[1] as i32;
            println!("Heart rate: {} BPM", hr_value);

            // Calculate and send heart rate percentage
            let hr_percent = (hr_value as f32 / MAX_HEART_RATE as f32).min(1.0);
            send_osc_message(osc_socket, OSC_PARAM_HR_PERCENT, OscType::Float(hr_percent))?;
        }
    }

    device.disconnect().await?;
    println!(
        "Disconnected from device: {:?}",
        device.properties().await?.unwrap().local_name.unwrap()
    );

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

fn send_osc_message(
    socket: &UdpSocket,
    address: &str,
    value: OscType,
) -> Result<(), Box<dyn Error>> {
    let packet = OscPacket::Message(rosc::OscMessage {
        addr: address.to_string(),
        args: vec![value],
    });
    let buf = rosc::encoder::encode(&packet)?;
    socket.send_to(&buf, OSC_ADDRESS)?;
    Ok(())
}
