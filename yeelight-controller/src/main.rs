use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::str::FromStr;
use std::time::Duration;

use log::{debug, error, info};
use paho_mqtt::Message;
use serde::{Serialize};

#[derive(Serialize)]
struct Command {
    id: u8,
    #[serde(flatten)]
    method: Method,
}

impl Command {
    pub const fn new(id: u8, method: Method) -> Self {
        Self { id, method }
    }
}

#[derive(Serialize)]
#[serde(tag = "method", rename_all = "snake_case")]
enum Method {
    GetProp { params: Vec<String> },
    SetBright { params: (u8, ) },
    SetPower { params: (Power, ) },
    Toggle { params: [(); 0] },
}

impl Method {
    pub const fn get_prop(params: Vec<String>) -> Method {
        Method::GetProp { params }
    }

    pub const fn set_brightness(brightness: u8) -> Method {
        Method::SetBright { params: (brightness, ) }
    }

    pub const fn set_power(power: Power) -> Method {
        Method::SetPower { params: (power, ) }
    }

    pub const TOGGLE: Method = Method::Toggle { params: [] };
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Power {
    On,
    Off,
}

impl FromStr for Power {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "on" => Ok(Self::On),
            "off" => Ok(Self::Off),
            _ => Err(format!("Invalid power value: {}", s)),
        }
    }
}

struct Device {
    socket: TcpStream,
    current_id: u8,
}

impl Device {
    const DEFAULT_PORT: u16 = 55443;

    fn new(hostname: String, port: u16) -> std::io::Result<Self> {
        let socket = TcpStream::connect((hostname, port)).expect("Failed to connect to device");
        Ok(Self { socket, current_id: 1 })
    }

    fn send_method(&mut self, method: Method) -> std::io::Result<String> {
        let command = Command::new(self.current_id, method);

        self.current_id += 1;

        serde_json::to_writer(&self.socket, &command)?;
        self.socket.write_all(b"\r\n")?;
        self.socket.flush()?;

        self.read_response()
    }

    fn read_response(&mut self) -> std::io::Result<String> {
        let mut lines = BufReader::new(&self.socket).lines();
        let response = lines.next().unwrap()?;
        Ok(response)
    }
}

#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    debug!("Starting yeelight controller");

    let device_ip = std::env::var("YEELIGHT_HOST")
        .expect("No host address provided. Set env YEELIGHT_HOST to the ip of the yeelight device.");

    let mut device = Device::new(device_ip, Device::DEFAULT_PORT)
        .expect("Failed to connect to yeelight device.");

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

        client.connect(connection_options).await
            .expect("Failed to connect to mqtt server");

        async fn subscribe_yeelight(client: &mut paho_mqtt::AsyncClient, topic: &str) {
            client.subscribe(format!("smart-home-system/yeelight/{}", topic), 1).await
                .expect(&format!("Failed to subscribe to topic: {}", topic));
        }

        subscribe_yeelight(&mut client, "set_power").await;
        subscribe_yeelight(&mut client, "set_brightness").await;
        subscribe_yeelight(&mut client, "toggle").await;

        while let Ok(message) = stream.recv().await {
            if let Some(message) = message {
                match message.topic() {
                    "smart-home-system/yeelight/set_power" => handle_mqtt_set_power(&mut device, message),
                    "smart-home-system/yeelight/set_brightness" => handle_mqtt_set_brightness(&mut device, message),
                    "smart-home-system/yeelight/toggle" => handle_mqtt_toggle(&mut device, message),
                    _ => error!("Received message on unknown topic: {}", message.topic()),
                }
            }
        };
    }).await.expect("Error creating tokio task");
}

fn handle_mqtt_toggle(device: &mut Device, message: Message) {
    info!(target: message.topic(), "Toggling yeelight device");
    device.send_method(Method::TOGGLE).expect("Could not send toggle method");
}

fn handle_mqtt_set_brightness(device: &mut Device, message: Message) {
    let payload = message.payload_str();

    if let Ok(brightness) = message.payload_str().parse() {
        info!(target: message.topic(), "Setting yeelight device brightness to: {:?}", brightness);
        device.send_method(Method::set_brightness(brightness)).expect("Could not send set_brightness method");
        return;
    }

    error!(target: message.topic(), "Received invalid payload: '{}'", payload);
}

fn handle_mqtt_set_power(device: &mut Device, message: Message) {
    let payload = message.payload_str();

    if let Ok(power) = Power::from_str(&payload) {
        info!(target: message.topic(), "Setting yeelight device power to: {:?}", power);
        device.send_method(Method::set_power(power)).expect("Could not send set_power method");
        return;
    }

    error!(target: message.topic(), "Received invalid payload: '{}'", payload);
}


#[cfg(test)]
mod tests {
    use crate::{Command, Method, Power};

    impl ToString for Command {
        fn to_string(&self) -> String {
            serde_json::to_string(self).unwrap()
        }
    }

    #[test]
    fn test_generate_json_packet() {
        let mut list = Vec::new();

        list.push((Command::new(1, Method::set_power(Power::On)),
                   "{\"id\":1,\"method\":\"set_power\",\"params\":[\"on\"]}"));

        list.push((Command::new(1, Method::set_brightness(50)),
                   "{\"id\":1,\"method\":\"set_bright\",\"params\":[50]}"));

        list.push((Command::new(1, Method::get_prop(vec!("power".to_string()))),
                   "{\"id\":1,\"method\":\"get_prop\",\"params\":[\"power\"]}"));

        list.push((Command::new(1, Method::TOGGLE),
                   "{\"id\":1,\"method\":\"toggle\",\"params\":[]}"));

        // Need a better way to do this

        for (command, expected) in list {
            match command.method {
                Method::GetProp { .. } => assert_eq!(command.to_string(), expected),
                Method::SetBright { .. } => assert_eq!(command.to_string(), expected),
                Method::SetPower { .. } => assert_eq!(command.to_string(), expected),
                Method::Toggle { .. } => assert_eq!(command.to_string(), expected),
            };
        }
    }
}
