use std::time::Duration;

use hap::{accessory::{AccessoryCategory, AccessoryInformation}, Config, MacAddress, Pin, Result, server::{IpServer, Server}, storage::{FileStorage, Storage}};
use hap::accessory::bridge::BridgeAccessory;
use hap::futures::future::join_all;

use crate::mqtt::MqttWrapper;

mod device;
mod mqtt;

async fn load_hap_rs_config(storage: &mut FileStorage) -> Result<Config> {
    let config = match storage.load_config().await {
        Ok(mut config) => {
            config.redetermine_local_ip();
            storage.save_config(&config).await?;
            config
        }
        Err(_) => {
            let config = Config {
                pin: Pin::new([1, 1, 1, 2, 2, 3, 3, 3])?,
                name: "smart-home-server-bridge".into(),
                device_id: MacAddress::from_bytes(&[20u8, 20u8, 30u8, 40u8, 50u8, 60u8]).unwrap(),
                category: AccessoryCategory::Bridge,
                ..Default::default()
            };
            storage.save_config(&config).await?;
            config
        }
    };
    Ok(config)
}

#[tokio::main]
async fn main() -> Result<()> {
    let mqtt_server_uri = std::env::var("MQTT_SERVER_URI")
        .expect("No mqtt server uri provided. Set env MQTT_SERVER_URI to the uri of the mqtt server.");

    let create_options = paho_mqtt::CreateOptionsBuilder::new()
        .server_uri(mqtt_server_uri)
        .client_id("homekit-mqtt-bridge")
        .finalize();

    let client = paho_mqtt::AsyncClient::new(create_options)
        .expect("Failed to create mqtt client");

    let mut connection_options = paho_mqtt::ConnectOptionsBuilder::new();

    if let Ok(username) = std::env::var("MQTT_USERNAME") {
        connection_options.user_name(username);
    }

    if let Ok(password) = std::env::var("MQTT_PASSWORD") {
        connection_options.password(password);
    }

    let connection_options = connection_options
        .keep_alive_interval(Duration::from_secs(20))
        .clean_session(true)
        .finalize();

    client.connect(connection_options).await
        .expect("Failed to connect to mqtt server");

    let mqtt_thread = tokio::spawn(async move {
        // for &topic in subscribe_topics.iter() {
        //     client.subscribe(topic, 1).await
        //         .unwrap_or_else(|_| panic!("Failed to subscribe to topic: {}", topic));
        // }
    });

    let bridge = BridgeAccessory::new(1, AccessoryInformation {
        name: "smart-home-system bridge".into(),
        ..Default::default()
    })?;

    let mut storage = FileStorage::current_dir().await?;

    let config = load_hap_rs_config(&mut storage).await?;

    let server = IpServer::new(config, storage).await?;
    server.add_accessory(bridge).await?;

    let mqtt_wrapper = MqttWrapper::new(client);

    let mut device = device::yeelight_device::YeelightDevice::new("yeelight".into());
    server.add_accessory(device.setup(2, &mqtt_wrapper)).await?;

    std::env::set_var("RUST_LOG", "hap=debug");
    env_logger::init();

    let hap_rs_handle = tokio::spawn(async move {
        let handle = server.run_handle();
        handle.await.expect("TODO: panic message");
    });

    join_all(vec![mqtt_thread, hap_rs_handle]).await;

    Ok(())
}
