use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use hap::accessory::HapAccessory;
use hap::characteristic::AsyncCharacteristicCallbacks;
use hap::characteristic::brightness::BrightnessCharacteristic;
use hap::characteristic::power_state::PowerStateCharacteristic;
use hap::futures::FutureExt;
use log::warn;
use paho_mqtt::Message;

use crate::mqtt::MqttWrapper;

pub mod yeelight_device;

pub struct InnerDevice<T, H> {
    pub name: String,
    pub device: T,
    h: PhantomData<H>,
}

impl<T, H> Clone for Device<T, H> {
    fn clone(&self) -> Self {
        Device {
            inner: self.inner.clone(),
        }
    }
}

pub struct Device<T, H> {
    inner: Arc<RwLock<InnerDevice<T, H>>>,
}

impl<D, H> Device<D, H> {
    pub(crate) fn new_device(name: String, device: D) -> Self {
        Device {
            inner: Arc::new(RwLock::new(InnerDevice {
                name,
                device,
                h: PhantomData,
            })),
        }
    }

    pub fn get_inner(&self) -> RwLockReadGuard<'_, InnerDevice<D, H>> {
        self.inner.read().unwrap()
    }

    pub fn get_inner_mut(&self) -> RwLockWriteGuard<'_, InnerDevice<D, H>> {
        self.inner.write().unwrap()
    }

    pub async fn characteristic<A>(&self, mqtt_client: MqttWrapper) -> anyhow::Result<A>
        where
            Self: Characteristic<A>,
    {
        self.get_value(mqtt_client)
    }

    pub fn set_characteristic<A>(&mut self, value: A, mqtt_client: MqttWrapper)
        where
            Self: Characteristic<A>,
    {
        self.set_value(value, mqtt_client);
    }

    pub async fn handle_message<A>(&mut self, message: Message, accessory: HapRsAccessory) -> Result<(), &'static str>
        where
            Self: Characteristic<A>,
    {
        self.handle_mqtt_message(message, accessory).await
    }
}

impl<D: Send + Sync + 'static, H: Send + Sync + 'static> Device<D, H> {
    fn setup_pointer<A>(self, topic: &str, mqtt_client: &mut MqttWrapper, lightbulb: HapRsAccessory)
        where
            Self: Characteristic<A>, {
        mqtt_client.subscribe(
            topic,
            Box::new(move |message: Message| {
                let mut self_clone = self.clone();
                let lightbulb = lightbulb.clone();
                Box::pin(async move {
                    if let Err(str) = self_clone.handle_message::<A>(message, lightbulb).await {
                        warn!("Error handling message: {}", str);
                    }
                })
            }),
        );
    }
}

impl<T, H> Device<T, H>
    where Self: Characteristic<Power>, H: Send + Sync + 'static, T: Send + Sync + 'static {
    pub fn setup_power(&self, mqtt_client: &MqttWrapper, power_state_characteristic: &mut PowerStateCharacteristic) {
        Self::setup_power_update(self.clone(), mqtt_client.clone(), power_state_characteristic);
        Self::setup_power_read(self.clone(), mqtt_client.clone(), power_state_characteristic);
    }

    fn setup_power_read(device: Device<T, H>, mqtt_client: MqttWrapper, power_state_characteristic: &mut PowerStateCharacteristic) {
        power_state_characteristic.on_read_async(Some(move || {
            let device = device.clone();
            let mqtt_client = mqtt_client.clone();
            async move {
                println!("Read of the power state characteristic was triggered.");
                device.characteristic::<Power>(mqtt_client.clone()).await
                    .map(|power| Some(power.0))
                    .or_else(|e| {
                        warn!("Read power error: {}", e);
                        Ok(None)
                    })
            }.boxed()
        }));
    }

    fn setup_power_update(device: Device<T, H>, mqtt_client: MqttWrapper, power_state_characteristic: &mut PowerStateCharacteristic) {
        power_state_characteristic.on_update_async(Some(move |current_val: bool, new_val: bool| {
            let mqtt_client = mqtt_client.clone();
            let mut device = device.clone();
            async move {
                let power = Power(new_val);

                println!("The power state was updated from {} to {}.", current_val, new_val);
                device.set_characteristic::<Power>(power, mqtt_client.clone());

                Ok(())
            }.boxed()
        }));
    }
}

impl<T, H> Device<T, H>
    where Self: Characteristic<Brightness>, H: Send + Sync + 'static, T: Send + Sync + 'static {
    pub fn setup_brightness(&self, mqtt_client: &MqttWrapper, brightness_characteristic: &mut BrightnessCharacteristic) {
        Self::setup_brightness_update(self.clone(), mqtt_client.clone(), brightness_characteristic);
        Self::setup_brightness_read(self.clone(), mqtt_client.clone(), brightness_characteristic);
    }

    fn setup_brightness_read(device: Device<T, H>, mqtt_client: MqttWrapper, brightness_characteristic: &mut BrightnessCharacteristic) {
        brightness_characteristic.on_read_async(Some(move || {
            let device = device.clone();
            let mqtt_client = mqtt_client.clone();
            async move {
                println!("Read of the brightness characteristic was triggered.");

                device.characteristic::<Brightness>(mqtt_client.clone()).await
                    .map(|brightness| Some(brightness.0 as i32))
                    .or_else(|e| {
                        warn!("Read brightness error: {}", e);
                        Ok(None)
                    })
            }.boxed()
        }));
    }

    fn setup_brightness_update(device: Device<T, H>, mqtt_client: MqttWrapper, brightness_characteristic: &mut BrightnessCharacteristic) {
        brightness_characteristic.on_update_async(Some(move |current_val: i32, new_val: i32| {
            let mqtt_client = mqtt_client.clone();
            let mut device = device.clone();
            async move {
                let brightness = Brightness(new_val as u8);

                println!("The brightness was updated from {} to {}.", current_val, new_val);
                device.set_characteristic::<Brightness>(brightness, mqtt_client.clone());

                Ok(())
            }.boxed()
        }));
    }
}

#[async_trait]
pub trait Characteristic<T> {
    fn get_value(&self, mqtt_client: MqttWrapper) -> anyhow::Result<T>;
    fn set_value(&mut self, value: T, mqtt_client: MqttWrapper);
    async fn handle_mqtt_message(&mut self, message: Message, accessory: HapRsAccessory) -> Result<(), &'static str>;
}

#[derive(Clone, Debug)]
pub struct Brightness(pub u8);

#[derive(Clone, Debug)]
pub struct Power(pub bool);

impl FromStr for Power {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "on" | "true" | "1" => Ok(Power(true)),
            "off" | "false" | "0" => Ok(Power(false)),
            _ => Err("Could not parse power state"),
        }
    }
}

impl ToString for Power {
    fn to_string(&self) -> String {
        match self.0 {
            true => "on".into(),
            false => "off".into(),
        }
    }
}

impl ToString for Brightness {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

type HapRsAccessory = Arc<hap::futures::lock::Mutex<Box<dyn HapAccessory>>>;
