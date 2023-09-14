use hap::accessory::AccessoryInformation;
use hap::accessory::lightbulb::LightbulbAccessory;
use paho_mqtt::{AsyncClient, Message};

use crate::device::{Brightness, Characteristic, Device, Power};

pub struct YeelightLightbulb;

pub type YeelightDevice = Device<YeelightLightbulb, LightbulbAccessory>;

impl YeelightDevice {
    pub fn new(name: String) -> Self {
        Device::new_device(name)
    }

    pub fn setup(&mut self, id: u64, mqtt_client: &AsyncClient) -> LightbulbAccessory {
        let mut lightbulb = LightbulbAccessory::new(id, AccessoryInformation {
            name: self.name().to_string(),
            ..Default::default()
        }).expect("The lightbulb accessory should be created successfully.");

        self.setup_power(mqtt_client, &mut lightbulb.lightbulb.power_state);
        self.setup_brightness(mqtt_client, lightbulb.lightbulb.brightness.as_mut().expect("The brightness characteristic should be created successfully."));

        lightbulb
    }
}

impl Characteristic<Brightness> for YeelightDevice {
    fn get_value(&self, mqtt_client: AsyncClient) -> Brightness {
        Brightness(85) // get value from mqtt
    }

    fn set_value(&mut self, value: Brightness, mqtt_client: AsyncClient) {
        let message = Message::new("smart-home-system/yeelight/brightness/set", value.to_string(), 1);
        mqtt_client.publish(message);
    }
}

impl Characteristic<Power> for YeelightDevice {
    fn get_value(&self, mqtt_client: AsyncClient) -> Power {
        Power(true) // get value from mqtt
    }

    fn set_value(&mut self, value: Power, mqtt_client: AsyncClient) {
        let message = Message::new("smart-home-system/yeelight/power/set", value.to_string(), 1);
        mqtt_client.publish(message);
    }
}