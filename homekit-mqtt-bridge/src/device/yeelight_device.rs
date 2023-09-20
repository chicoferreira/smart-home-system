use std::str::FromStr;

use async_trait::async_trait;
use hap::accessory::AccessoryInformation;
use hap::accessory::lightbulb::LightbulbAccessory;
use hap::HapType;
use hap::server::{IpServer, Server};
use paho_mqtt::Message;

use crate::device::{Brightness, Characteristic, Device, HapRsAccessory, Power};
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

    pub async fn setup(&mut self, id: u64, mqtt_client: &mut MqttWrapper, ip_server: &IpServer) {
        let mut lightbulb = LightbulbAccessory::new(id, AccessoryInformation {
            name: self.get_inner().name.to_string(),
            ..Default::default()
        }).expect("The lightbulb accessory should be created successfully.");

        self.setup_power(mqtt_client, &mut lightbulb.lightbulb.power_state);
        self.setup_brightness(mqtt_client, lightbulb.lightbulb.brightness.as_mut().expect("The brightness characteristic should be created successfully."));

        let accessory = ip_server.add_accessory(lightbulb).await.expect("The lightbulb accessory should be added successfully.");

        self.clone().setup_pointer::<Brightness>("smart-home-system/yeelight/brightness", mqtt_client, accessory.clone());
        self.clone().setup_pointer::<Power>("smart-home-system/yeelight/power", mqtt_client, accessory.clone());
    }
}

#[async_trait]
impl Characteristic<Brightness> for YeelightDevice {
    fn get_value(&self, _mqtt_client: MqttWrapper) -> anyhow::Result<Brightness> {
        Ok(self.get_inner().device.brightness.clone())
    }

    fn set_value(&mut self, value: Brightness, mut mqtt_client: MqttWrapper) {
        self.get_inner_mut().device.brightness = value.clone();
        mqtt_client.publish("smart-home-system/yeelight/brightness/set", value.to_string())
    }

    async fn handle_mqtt_message(&mut self, message: Message, accessory: HapRsAccessory) -> Result<(), &'static str> {
        let payload = message.payload_str();
        let brightness = Brightness(payload.parse::<u8>().map_err(|_| "Could not parse brightness")?);

        let mut lightbulb = accessory.lock().await;
        let lightbulb_service = lightbulb.get_mut_service(HapType::Lightbulb)
            .expect("The lightbulb service should be created successfully.");

        let brightness_characteristic = lightbulb_service
            .get_mut_characteristic(HapType::Brightness)
            .unwrap();

        self.get_inner_mut().device.brightness = brightness.clone();
        brightness_characteristic.set_value(brightness.0.into()).await.expect("TODO: panic message");

        Ok(())
    }
}

#[async_trait]
impl Characteristic<Power> for YeelightDevice {
    fn get_value(&self, _mqtt_client: MqttWrapper) -> anyhow::Result<Power> {
        Ok(self.get_inner().device.power_state.clone())
    }

    fn set_value(&mut self, value: Power, mut mqtt_client: MqttWrapper) {
        self.get_inner_mut().device.power_state = value.clone();
        mqtt_client.publish("smart-home-system/yeelight/power/set", value.to_string());
    }

    async fn handle_mqtt_message(&mut self, message: Message, accessory: HapRsAccessory) -> Result<(), &'static str> {
        let payload = message.payload_str();
        let power = Power::from_str(&payload)?;

        let mut lightbulb = accessory.lock().await;
        let lightbulb_service = lightbulb.get_mut_service(HapType::Lightbulb)
            .expect("The lightbulb service should be created successfully.");

        let power_characteristic = lightbulb_service
            .get_mut_characteristic(HapType::PowerState)
            .expect("The power characteristic should be created successfully.");

        self.get_inner_mut().device.power_state = power.clone();
        power_characteristic.set_value(power.0.into()).await.expect("TODO: panic message");

        Ok(())
    }
}