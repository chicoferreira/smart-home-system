use std::marker::PhantomData;
use std::sync::{Arc, RwLock};

use hap::characteristic::AsyncCharacteristicCallbacks;
use hap::characteristic::brightness::BrightnessCharacteristic;
use hap::characteristic::power_state::PowerStateCharacteristic;
use hap::futures::FutureExt;
use log::warn;

use crate::mqtt::MqttWrapper;

pub mod yeelight_device;

struct InnerDevice<T, H> {
    name: String,
    device: T,
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

    pub fn name(&self) -> &str {
        &self.inner.read().unwrap().name
    }

    pub fn get_device(&self) -> &D {
        &self.inner.read().unwrap().device
    }

    pub fn get_mut_device(&mut self) -> &mut D {
        &mut self.inner.write().unwrap().device
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

pub trait Characteristic<T> {
    fn get_value(&self, mqtt_client: MqttWrapper) -> anyhow::Result<T>;
    fn set_value(&mut self, value: T, mqtt_client: MqttWrapper);
}

#[derive(Clone, Debug)]
pub struct Brightness(pub u8);

#[derive(Clone, Debug)]
pub struct Power(pub bool);

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
