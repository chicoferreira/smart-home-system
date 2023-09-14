pub mod yeelight_device;

use std::marker::PhantomData;
use std::sync::Arc;

use hap::characteristic::AsyncCharacteristicCallbacks;
use hap::characteristic::brightness::BrightnessCharacteristic;
use hap::characteristic::power_state::PowerStateCharacteristic;
use hap::futures::FutureExt;
use paho_mqtt::AsyncClient;

pub struct InnerDevice<T, H> {
    pub name: String,
    pub t: PhantomData<T>,
    pub h: PhantomData<H>,
}

impl<T, H> Clone for Device<T, H> {
    fn clone(&self) -> Self {
        Device {
            inner: self.inner.clone(),
        }
    }
}

pub struct Device<T, H> {
    pub inner: Arc<InnerDevice<T, H>>,
}

impl<T, H> Device<T, H> {
    pub(crate) fn new_device(name: String) -> Self {
        Device {
            inner: Arc::new(InnerDevice {
                name,
                t: PhantomData,
                h: PhantomData,
            }),
        }
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }

    pub fn characteristic<A>(&self, mqtt_client: AsyncClient) -> A
        where
            Self: Characteristic<A>,
    {
        self.get_value(mqtt_client)
    }

    pub fn set_characteristic<A>(&mut self, value: A, mqtt_client: AsyncClient)
        where
            Self: Characteristic<A>,
    {
        self.set_value(value, mqtt_client);
    }
}

impl<T, H> Device<T, H>
    where Self: Characteristic<Power>, H: Send + Sync + 'static, T: Send + Sync + 'static {
    pub fn setup_power(&self, mqtt_client: &AsyncClient, power_state_characteristic: &mut PowerStateCharacteristic) {
        Self::setup_power_update(self.clone(), mqtt_client.clone(), power_state_characteristic);
        Self::setup_power_read(self.clone(), mqtt_client.clone(), power_state_characteristic);
    }

    fn setup_power_read(device: Device<T, H>, mqtt_client: AsyncClient, power_state_characteristic: &mut PowerStateCharacteristic) {
        power_state_characteristic.on_read_async(Some(move || {
            let device = device.clone();
            let mqtt_client = mqtt_client.clone();
            async move {
                println!("Read of the power state characteristic was triggered.");
                let power = device.characteristic::<Power>(mqtt_client.clone());

                Ok(Some(power.0))
            }.boxed()
        }));
    }

    fn setup_power_update(device: Device<T, H>, mqtt_client: AsyncClient, power_state_characteristic: &mut PowerStateCharacteristic) {
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
    pub fn setup_brightness(&self, mqtt_client: &AsyncClient, brightness_characteristic: &mut BrightnessCharacteristic) {
        Self::setup_brightness_update(self.clone(), mqtt_client.clone(), brightness_characteristic);
        Self::setup_brightness_read(self.clone(), mqtt_client.clone(), brightness_characteristic);
    }

    fn setup_brightness_read(device: Device<T, H>, mqtt_client: AsyncClient, brightness_characteristic: &mut BrightnessCharacteristic) {
        brightness_characteristic.on_read_async(Some(move || {
            let device = device.clone();
            let mqtt_client = mqtt_client.clone();
            async move {
                println!("Read of the brightness characteristic was triggered.");

                let brightness = device.characteristic::<Brightness>(mqtt_client.clone());
                Ok(Some(brightness.0 as i32))
            }.boxed()
        }));
    }

    fn setup_brightness_update(device: Device<T, H>, mqtt_client: AsyncClient, brightness_characteristic: &mut BrightnessCharacteristic) {
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
    fn get_value(&self, mqtt_client: AsyncClient) -> T;
    fn set_value(&mut self, value: T, mqtt_client: AsyncClient);
}

pub struct Brightness(pub u8);

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
