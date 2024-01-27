use std::str::FromStr;
use std::time::Duration;

use log::{error, info, warn};
use paho_mqtt::{AsyncClient, Message};
use tokio::sync::mpsc;

use crate::{discovery, MQTT_BRIGHTNESS_PUBLISH_TOPIC, MQTT_POWER_PUBLISH_TOPIC};
use crate::yeelight::{Device, Method, Notification, Power, ResponseResult};

pub struct Application {
    client: AsyncClient,
    device: Device,
    handle: tokio::task::JoinHandle<()>,
}

#[derive(Debug)]
pub struct DeviceFilters {
    pub id: Option<String>,
    pub model: Option<String>,
}

impl DeviceFilters {
    fn matches(&self, device: &discovery::DiscoveryResponse) -> bool {
        self.id.as_ref().map_or(true, |id| device.id == *id) &&
            self.model.as_ref().map_or(true, |model| device.model == *model)
    }
}

impl Drop for Application {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl Application {
    pub async fn new(client: AsyncClient, filter: DeviceFilters) -> Self {
        let (device, mut notification_receiver) = Self::find_device(filter).await;

        let c = client.clone();

        let handle = tokio::spawn(async move {
            while let Some(notification) = notification_receiver.recv().await {
                handle_yeelight_notification(&c, notification);
            }
        });

        Self { client, device, handle }
    }

    pub async fn find_device(filter: DeviceFilters) -> (Device, mpsc::Receiver<Notification>) {
        let (sender, receiver) = mpsc::channel(1);

        loop {
            let result = discovery::discover(Duration::from_secs(3)).await;
            match result {
                Ok(discovery) => {
                    let device = discovery.into_iter().find(|device| filter.matches(device));

                    if let Some(device) = device {
                        let address = device.location.trim_start_matches("yeelight://").to_string();
                        info!("Connecting to yeelight device at {}...", address);
                        return (Device::new(address, sender).await.unwrap(), receiver);
                    } else {
                        warn!("No yeelight device found matching filter {filter:?}. Retrying in 30 seconds...");
                    }
                }
                Err(e) => warn!("Yeelight discovery failed: {}. Retring in 30 seconds...", e)
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    }

    pub async fn handle_mqtt_toggle(&mut self, message: &Message) {
        info!("[{}] Toggling yeelight device",  message.topic());
        self.device.send_method(Method::TOGGLE).await.unwrap();
    }

    pub async fn handle_mqtt_brightness_set(&mut self, message: &Message) {
        let payload = message.payload_str();

        if let Ok(brightness) = message.payload_str().parse::<u8>() {
            let brightness = brightness.max(1).min(100);

            info!("[{}] Setting yeelight device brightness to: {:?}",  message.topic(), brightness);
            self.device.send_method(Method::set_brightness(brightness)).await.expect("Could not send set_brightness method");
            return;
        }

        error!("[{}] Received invalid payload: '{}'", message.topic(), payload);
    }

    pub async fn handle_mqtt_set_power(&mut self, message: &Message) {
        let payload = message.payload_str();

        if let Ok(power) = Power::from_str(&payload) {
            info!("[{}] Setting yeelight device power to: {:?}", message.topic(), power);
            self.device.send_method(Method::set_power(power)).await.expect("Could not send set_power method");
            return;
        }

        error!("[{}] Received invalid payload: '{}'", message.topic(), payload);
    }

    pub async fn handle_mqtt_get_power(&mut self) {
        let response = self.device.send_method(Method::get_prop(vec!("power".into()))).await.expect("Could not send get_prop method");

        info!("Getting yeelight device power: {:?}", response);

        match response.result {
            ResponseResult::Success(response) => {
                if let Some(power) = response.first() {
                    mqtt_publish_power(&self.client, Power::from_str(power).unwrap());
                };
            }
            ResponseResult::Error { .. } => {}
        }
    }

    pub async fn handle_mqtt_get_brightness(&mut self) {
        let response = self.device.send_method(Method::get_prop(vec!("bright".into()))).await.expect("Could not send get_prop method");

        info!("Getting yeelight device brightness: {:?}", response);

        match response.result {
            ResponseResult::Success(response) => {
                if let Some(brightness) = response.first() {
                    mqtt_publish_brightness(&self.client, brightness.parse().unwrap());
                };
            }
            ResponseResult::Error { .. } => {}
        }
    }
}

fn handle_yeelight_notification(client: &AsyncClient, notification: Notification) {
    info!("Received notification: {:?}", notification);

    notification.params.iter().for_each(|(key, value)| {
        match key.as_ref() {
            "power" => {
                if let Ok(power) = Power::from_str(value.as_str().unwrap()) {
                    info!("Yeelight device power changed to: {:?}", power);
                    mqtt_publish_power(client, power);
                } else {
                    warn!("Couldn't parse power value from '{:?}' received from yeelight", value);
                }
            }
            "bright" => {
                if let Some(value) = value.as_u64() {
                    info!("Yeelight device brightness changed to: {:?}", value);
                    mqtt_publish_brightness(client, value as u8);
                } else {
                    warn!("Couldn't parse brighness value from '{:?}' received from yeelight", value);
                }
            }
            _ => {}
        }
    });
}

fn mqtt_publish_power(client: &AsyncClient, power: Power) {
    let message = Message::new_retained(MQTT_POWER_PUBLISH_TOPIC, power.to_string(), 1);
    client.publish(message);
}

fn mqtt_publish_brightness(client: &AsyncClient, brightness: u8) {
    let message = Message::new_retained(MQTT_BRIGHTNESS_PUBLISH_TOPIC, brightness.to_string(), 1);
    client.publish(message);
}