use anyhow::anyhow;
use async_trait::async_trait;
use hap::accessory::AccessoryInformation;
use hap::accessory::lightbulb::LightbulbAccessory;

use crate::device::{Brightness, Characteristic, Device, Power};
use crate::mqtt::MqttWrapper;

pub struct YeelightLightbulb;

pub type YeelightDevice = Device<YeelightLightbulb, LightbulbAccessory>;

impl YeelightDevice {
    pub fn new(name: String) -> Self {
        Device::new_device(name)
    }

    pub fn setup(&mut self, id: u64, mqtt_client: &MqttWrapper) -> LightbulbAccessory {
        let mut lightbulb = LightbulbAccessory::new(id, AccessoryInformation {
            name: self.name().to_string(),
            ..Default::default()
        }).expect("The lightbulb accessory should be created successfully.");

        self.setup_power(mqtt_client, &mut lightbulb.lightbulb.power_state);
        self.setup_brightness(mqtt_client, lightbulb.lightbulb.brightness.as_mut().expect("The brightness characteristic should be created successfully."));

        lightbulb
    }
}

#[async_trait]
impl Characteristic<Brightness> for YeelightDevice {
    async fn get_value(&self, mut mqtt_client: MqttWrapper) -> anyhow::Result<Brightness> {
        let brightness = mqtt_client.get("smart-home-system/yeelight/brightness/get", "smart-home-system/yeelight/brightness").await
            .map_err(|_| anyhow!("get_value for power timeout"))?;

        let brightness_value = brightness.parse::<u8>()
            .map_err(|_| anyhow!("Couldn't parse brightness from '{}'", brightness))?;

        Ok(Brightness(brightness_value))
    }

    fn set_value(&mut self, value: Brightness, mut mqtt_client: MqttWrapper) {
        mqtt_client.publish("smart-home-system/yeelight/brightness/set", value.to_string())
    }
}

#[async_trait]
impl Characteristic<Power> for YeelightDevice {
    async fn get_value(&self, mut mqtt_client: MqttWrapper) -> anyhow::Result<Power> {
        let power = mqtt_client.get("smart-home-system/yeelight/power/get", "smart-home-system/yeelight/power").await
            .map_err(|_| anyhow!("get_value for power timeout"))?;

        let power_value = power.parse::<bool>()
            .map_err(|_| anyhow!("Couldn't parse power from '{}'", power))?;

        Ok(Power(power_value))
    }

    fn set_value(&mut self, value: Power, mut mqtt_client: MqttWrapper) {
        mqtt_client.publish("smart-home-system/yeelight/power/set", value.to_string());
    }
}