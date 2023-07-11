use std::str::FromStr;
use std::time::Duration;

use log::{error, info};
use paho_mqtt::Message;

use yeelight::{Device, Method, Power};

mod yeelight;

const MQTT_SET_POWER_TOPIC: &str = "smart-home-system/yeelight/set_power";
const MQTT_SET_BRIGHTNESS_TOPIC: &str = "smart-home-system/yeelight/set_brightness";
const MQTT_TOGGLE_TOPIC: &str = "smart-home-system/yeelight/toggle";

#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    info!("Starting yeelight controller");

    let device_ip = std::env::var("YEELIGHT_HOST")
        .expect("No host address provided. Set env YEELIGHT_HOST to the ip of the yeelight device.");

    info!("Connecting to yeelight device at {}...", device_ip);

    let mut device = Device::new(device_ip, Device::DEFAULT_PORT).await
        .expect("Failed to connect to yeelight device.");

    info!("Connected to yeelight device.");

    let mqtt_server_uri = std::env::var("MQTT_SERVER_URI")
        .expect("No mqtt server uri provided. Set env MQTT_SERVER_URI to the uri of the mqtt server.");

    let create_options = paho_mqtt::CreateOptionsBuilder::new()
        .server_uri(mqtt_server_uri)
        .client_id("yeelight-controller")
        .finalize();


    let mut client = paho_mqtt::AsyncClient::new(create_options)
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

    tokio::spawn(async move {
        let stream = client.get_stream(10);

        info!("Connecting to mqtt server...");
        client.connect(connection_options).await
            .expect("Failed to connect to mqtt server");

        let subscribe_topics = [MQTT_SET_POWER_TOPIC, MQTT_SET_BRIGHTNESS_TOPIC, MQTT_TOGGLE_TOPIC];

        for &topic in subscribe_topics.iter() {
            info!("Subscribing to mqtt topic: {}", topic);
            client.subscribe(topic, 1).await
                .unwrap_or_else(|_| panic!("Failed to subscribe to topic: {}", topic));
        }

        info!("Waiting for mqtt messages...");

        while let Ok(message) = stream.recv().await {
            if let Some(message) = message {
                match message.topic() {
                    MQTT_SET_POWER_TOPIC => handle_mqtt_set_power(&mut device, &message).await,
                    MQTT_SET_BRIGHTNESS_TOPIC => handle_mqtt_set_brightness(&mut device, &message).await,
                    MQTT_TOGGLE_TOPIC => handle_mqtt_toggle(&mut device, &message).await,
                    _ => error!("Received message for unknown topic: {}", message.topic()),
                }
            }
        };
    }).await.expect("Error creating tokio task");
}

async fn handle_mqtt_toggle(device: &mut Device, message: &Message) {
    info!("[{}] Toggling yeelight device",  message.topic());
    device.send_method(Method::TOGGLE).await.expect("Could not send toggle method");
}

async fn handle_mqtt_set_brightness(device: &mut Device, message: &Message) {
    let payload = message.payload_str();

    if let Ok(brightness) = message.payload_str().parse() {
        info!("[{}] Setting yeelight device brightness to: {:?}",  message.topic(), brightness);
        device.send_method(Method::set_brightness(brightness)).await.expect("Could not send set_brightness method");
        return;
    }

    error!("[{}] Received invalid payload: '{}'", message.topic(), payload);
}

async fn handle_mqtt_set_power(device: &mut Device, message: &Message) {
    let payload = message.payload_str();

    if let Ok(power) = Power::from_str(&payload) {
        info!("[{}] Setting yeelight device power to: {:?}", message.topic(), power);
        device.send_method(Method::set_power(power)).await.expect("Could not send set_power method");
        return;
    }

    error!("[{}] Received invalid payload: '{}'", message.topic(), payload);
}