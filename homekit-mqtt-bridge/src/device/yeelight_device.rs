use hap::accessory::AccessoryInformation;
use hap::accessory::lightbulb::LightbulbAccessory;
use hap::characteristic::HapCharacteristic;
use paho_mqtt::Message;

use crate::device::{Brightness, Characteristic, Device, Power};
use crate::mqtt::MqttWrapper;

pub struct YeelightLightbulb {
    pub power_state: Power,
    pub brightness: Brightness,
}

pub type YeelightDevice = Device<YeelightLightbulb, LightbulbAccessory>;

impl YeelightDevice {
    pub fn new(name: String) -> Self {
        Device::new_device(name, YeelightLightbulb {
            power_state: Power(false),
            brightness: Brightness(0),
        })
    }

    pub fn setup(&mut self, id: u64, mqtt_client: &mut MqttWrapper) -> LightbulbAccessory {
        let mut lightbulb = LightbulbAccessory::new(id, AccessoryInformation {
            name: self.name().to_string(),
            ..Default::default()
        }).expect("The lightbulb accessory should be created successfully.");

        self.setup_power(mqtt_client, &mut lightbulb.lightbulb.power_state);
        self.setup_brightness(mqtt_client, lightbulb.lightbulb.brightness.as_mut().expect("The brightness characteristic should be created successfully."));

        mqtt_client.subscribe("smart-home-system/yeelight/brightness", Box::new(|message: &Message| Box::new(async {
            let payload = message.payload_str();
            let brightness = Brightness(payload.parse::<u8>().expect("The payload should be a u8."));
            lightbulb.lightbulb.brightness.unwrap().set_value(brightness.0.into()).await;
        })));

        lightbulb
    }
}

impl Characteristic<Brightness> for YeelightDevice {
    fn get_value(&self, _mqtt_client: MqttWrapper) -> anyhow::Result<Brightness> {
        Ok(self.get_device().brightness.clone())
    }

    fn set_value(&mut self, value: Brightness, mut mqtt_client: MqttWrapper) {
        self.get_mut_device().brightness = value.clone();
        mqtt_client.publish("smart-home-system/yeelight/brightness/set", value.to_string())
    }
}

impl Characteristic<Power> for YeelightDevice {
    fn get_value(&self, _mqtt_client: MqttWrapper) -> anyhow::Result<Power> {
        Ok(self.get_device().power_state.clone())
    }

    fn set_value(&mut self, value: Power, mut mqtt_client: MqttWrapper) {
        self.get_mut_device().power_state = value.clone();
        mqtt_client.publish("smart-home-system/yeelight/power/set", value.to_string());
    }
}