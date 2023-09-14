use std::str::FromStr;
use std::time::Duration;

use log::{error, info};
use paho_mqtt::{AsyncClient, Message};

use yeelight::{Device, Method, Power};

use crate::yeelight::ResponseResult;

mod yeelight;

const MQTT_SET_BRIGHTNESS_TOPIC: &str = "smart-home-system/yeelight/brightness/set";
const MQTT_GET_BRIGHTNESS_TOPIC: &str = "smart-home-system/yeelight/brightness/get";
const MQTT_BRIGHTNESS_PUBLISH_TOPIC: &str = "smart-home-system/yeelight/brightness";
const MQTT_SET_POWER_TOPIC: &str = "smart-home-system/yeelight/power/set";
const MQTT_GET_POWER_TOPIC: &str = "smart-home-system/yeelight/power/get";
const MQTT_POWER_PUBLISH_TOPIC: &str = "smart-home-system/yeelight/power";
const MQTT_TOGGLE_TOPIC: &str = "smart-home-system/yeelight/toggle";

#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let mqtt_server_uri = std::env::var("MQTT_SERVER_URI")
        .expect("No mqtt server uri provided. Set env MQTT_SERVER_URI to the uri of the mqtt server.");

    let create_options = paho_mqtt::CreateOptionsBuilder::new()
        .server_uri(mqtt_server_uri)
        .client_id("yeelight-controller")
        .finalize();


    let mut client = AsyncClient::new(create_options)
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

        let subscribe_topics = [
            MQTT_SET_POWER_TOPIC,
            MQTT_SET_BRIGHTNESS_TOPIC,
            MQTT_TOGGLE_TOPIC,
            MQTT_GET_POWER_TOPIC,
            MQTT_GET_BRIGHTNESS_TOPIC];

        for &topic in subscribe_topics.iter() {
            info!("Subscribing to mqtt topic: {}", topic);
            client.subscribe(topic, 1).await
                .unwrap_or_else(|_| panic!("Failed to subscribe to topic: {}", topic));
        }

        info!("Starting yeelight controller");

        let device_ip = std::env::var("YEELIGHT_HOST")
            .expect("No host address provided. Set env YEELIGHT_HOST to the ip of the yeelight device.");

        info!("Connecting to yeelight device at {}...", device_ip);

        let mut device = Device::new(device_ip, Device::DEFAULT_PORT, handle_yeelight_notification, &client.clone()).await
            .expect("Failed to connect to yeelight device.");

        info!("Connected to yeelight device.");

        info!("Waiting for mqtt messages...");

        while let Ok(message) = stream.recv().await {
            if let Some(message) = message {
                match message.topic() {
                    MQTT_SET_POWER_TOPIC => handle_mqtt_set_power(&mut device, &message).await,
                    MQTT_SET_BRIGHTNESS_TOPIC => handle_mqtt_brightness_set(&mut device, &message).await,
                    MQTT_TOGGLE_TOPIC => handle_mqtt_toggle(&mut device, &message).await,
                    MQTT_GET_POWER_TOPIC => handle_mqtt_get_power(&mut device, &client).await,
                    MQTT_GET_BRIGHTNESS_TOPIC => handle_mqtt_get_brightness(&mut device, &client).await,
                    _ => error!("Received message for unknown topic: {}", message.topic()),
                }
            }
        };
    }).await.expect("Error creating tokio task");
}

fn handle_yeelight_notification(notification: yeelight::Notification, client: &AsyncClient) {
    info!("Received notification: {:?}", notification);

    notification.params.iter().for_each(|(key, value)| {
        match key.as_ref() {
            "power" => {
                info!("Yeelight device power changed to: {:?}", value);
                mqtt_publish_power(client, Power::from_str(value.as_str().unwrap()).unwrap());
            }
            "bright" => {
                info!("Yeelight device brightness changed to: {:?}", value);
                mqtt_publish_brightness(client, value.as_u64().unwrap() as u8);
            }
            _ => {}
        }
    });
}

async fn handle_mqtt_toggle(device: &mut Device, message: &Message) {
    info!("[{}] Toggling yeelight device",  message.topic());
    device.send_method(Method::TOGGLE).await.unwrap();
}

async fn handle_mqtt_brightness_set(device: &mut Device, message: &Message) {
    let payload = message.payload_str();

    if let Ok(brightness) = message.payload_str().parse::<u8>() {
        let brightness = brightness.max(1).min(100);

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

async fn handle_mqtt_get_power(device: &mut Device, client: &AsyncClient) {
    let response = device.send_method(Method::get_prop(vec!("power".into()))).await.expect("Could not send get_prop method");

    info!("Getting yeelight device power: {:?}", response);

    match response.result {
        ResponseResult::Success(response) => {
            if let Some(power) = response.first() {
                mqtt_publish_power(client, Power::from_str(power).unwrap());
            };
        }
        ResponseResult::Error { .. } => {}
    }
}

async fn handle_mqtt_get_brightness(device: &mut Device, client: &AsyncClient) {
    let response = device.send_method(Method::get_prop(vec!("bright".into()))).await.expect("Could not send get_prop method");

    info!("Getting yeelight device brightness: {:?}", response);

    match response.result {
        ResponseResult::Success(response) => {
            if let Some(brightness) = response.first() {
                mqtt_publish_brightness(client, brightness.parse().unwrap());
            };
        }
        ResponseResult::Error { .. } => {}
    }
}

fn mqtt_publish_power(client: &AsyncClient, power: Power) {
    let message = Message::new_retained(MQTT_POWER_PUBLISH_TOPIC, power.to_string(), 1);
    client.publish(message);
}

fn mqtt_publish_brightness(client: &AsyncClient, brightness: u8) {
    let message = Message::new_retained(MQTT_BRIGHTNESS_PUBLISH_TOPIC, brightness.to_string(), 1);
    client.publish(message);
}