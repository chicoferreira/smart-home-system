use anyhow::Context;
use log::{error, info};

use crate::application::{Application, DeviceFilters};
use crate::mqtt::connect_mqtt;

mod yeelight;
mod application;
mod mqtt;
mod discovery;

const MQTT_SET_BRIGHTNESS_TOPIC: &str = "smart-home-system/yeelight/brightness/set";
const MQTT_GET_BRIGHTNESS_TOPIC: &str = "smart-home-system/yeelight/brightness/get";
const MQTT_BRIGHTNESS_PUBLISH_TOPIC: &str = "smart-home-system/yeelight/brightness";
const MQTT_SET_POWER_TOPIC: &str = "smart-home-system/yeelight/power/set";
const MQTT_GET_POWER_TOPIC: &str = "smart-home-system/yeelight/power/get";
const MQTT_POWER_PUBLISH_TOPIC: &str = "smart-home-system/yeelight/power";
const MQTT_TOGGLE_TOPIC: &str = "smart-home-system/yeelight/toggle";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let subscribe_topics = [
        MQTT_SET_POWER_TOPIC,
        MQTT_SET_BRIGHTNESS_TOPIC,
        MQTT_TOGGLE_TOPIC,
        MQTT_GET_POWER_TOPIC,
        MQTT_GET_BRIGHTNESS_TOPIC];

    let mqtt_server_uri = std::env::var("MQTT_SERVER_URI")
        .context("No mqtt server uri provided. Set env MQTT_SERVER_URI to the uri of the mqtt server.")?;

    let (client, stream) = connect_mqtt(
        &subscribe_topics,
        mqtt_server_uri,
        std::env::var("MQTT_USERNAME").ok(),
        std::env::var("MQTT_PASSWORD").ok(),
    ).await.context("Failed to connect to mqtt server")?;

    info!("Starting yeelight controller");

    let mut application = Application::new(client, DeviceFilters {
        id: std::env::var("YEELIGHT_ID").ok(),
        model: std::env::var("YEELIGHT_MODEL").ok(),
    }).await;

    info!("Connected to yeelight device.");

    info!("Waiting for mqtt messages...");

    while let Ok(message) = stream.recv().await {
        if let Some(message) = message {
            match message.topic() {
                MQTT_SET_POWER_TOPIC => application.handle_mqtt_set_power(&message).await,
                MQTT_SET_BRIGHTNESS_TOPIC => application.handle_mqtt_brightness_set(&message).await,
                MQTT_TOGGLE_TOPIC => application.handle_mqtt_toggle(&message).await,
                MQTT_GET_POWER_TOPIC => application.handle_mqtt_get_power().await,
                MQTT_GET_BRIGHTNESS_TOPIC => application.handle_mqtt_get_brightness().await,
                _ => error!("Received message for unknown topic: {}", message.topic()),
            }
        }
    };

    Ok(())
}