use std::collections::HashMap;
use std::fmt::Display;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use dashmap::DashMap;
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

#[derive(Serialize)]
pub struct Command {
    id: u64,
    #[serde(flatten)]
    method: Method,
}

impl Command {
    pub const fn new(id: u64, method: Method) -> Self {
        Self { id, method }
    }
}

#[derive(Serialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum Method {
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

impl Display for Power {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::On => "on",
            Self::Off => "off",
        })
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum YeelightMessage {
    Response(Response),
    Notification(Notification),
}

#[derive(Deserialize, Debug, Clone)]
pub struct Response {
    pub id: u64,
    #[serde(flatten)]
    pub result: ResponseResult,
}

#[derive(Deserialize, PartialEq, Debug, Clone)]
pub enum ResponseResult {
    #[serde(rename = "result")]
    Success(Vec<String>),

    #[serde(rename = "error")]
    Error { code: i64, message: String },
}

impl FromStr for Response {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

#[derive(Deserialize, Debug)]
pub struct Notification {
    pub method: String,
    pub params: HashMap<String, Value>,
}

impl FromStr for Notification {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

pub struct Device {
    current_id: AtomicU64,
    write_half: OwnedWriteHalf,
    responses: Arc<DashMap<u64, oneshot::Sender<Response>>>,
    read_handle: JoinHandle<()>,
}

impl Device {
    pub const DEFAULT_PORT: u16 = 55443;

    pub async fn new(address: String, mut notification_handler: mpsc::Sender<Notification>) -> anyhow::Result<Self> {
        let (read_half, write_half) = TcpStream::connect(address).await?.into_split();

        let responses: Arc<DashMap<u64, oneshot::Sender<Response>>> = Arc::new(DashMap::new());

        let arc = responses.clone();

        let read_handle = tokio::spawn(async move {
            let mut read_half = BufReader::new(read_half);
            let mut buffer = String::new();
            while read_half.read_line(&mut buffer).await.unwrap() > 0 {
                if !buffer.is_empty() {
                    Self::process_incoming_message(&arc, &mut buffer, &mut notification_handler).await;
                }
                buffer.clear();
            }
        });

        Ok(Self { write_half, current_id: AtomicU64::new(0), responses: responses.clone(), read_handle })
    }

    async fn process_incoming_message(
        wait_map: &Arc<DashMap<u64, oneshot::Sender<Response>>>,
        content: &mut str, notification_sender:
        &mut mpsc::Sender<Notification>,
    ) {
        let message: YeelightMessage = match serde_json::from_str(content) {
            Ok(message) => message,
            Err(error) => {
                error!("Failed to parse incoming message: {}: {}", error, content);
                return;
            }
        };

        match message {
            YeelightMessage::Response(response) => {
                if let Some((_, sender)) = wait_map.remove(&response.id) {
                    sender.send(response).unwrap();
                }
            }
            YeelightMessage::Notification(notification) => {
                notification_sender.send(notification).await.unwrap();
            }
        }
    }

    pub async fn send_method(&mut self, method: Method) -> anyhow::Result<Response> {
        let command = self.new_command(method).await;

        self.write_half.write_all(&serde_json::to_vec(&command)?).await?;
        self.write_half.write_all(b"\r\n").await?;
        self.write_half.flush().await?;

        self.read_response(command.id).await
    }

    async fn new_command(&mut self, method: Method) -> Command {
        let current_id = self.current_id.get_mut();
        *current_id += 1;

        Command::new(*current_id, method)
    }

    async fn read_response(&mut self, id: u64) -> anyhow::Result<Response> {
        let (sender, receiver) = oneshot::channel();
        self.responses.insert(id, sender);

        let response = tokio::time::timeout(Duration::from_secs(5), receiver).await;

        if let Ok(Ok(response)) = response {
            return Ok(response);
        }

        anyhow::bail!("{} id timedout", id)
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        self.read_handle.abort();
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Display;
    use std::str::FromStr;

    use crate::yeelight::{Command, Method, Notification, Power, Response, ResponseResult};

    impl Display for Command {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", serde_json::to_string(self).unwrap())
        }
    }

    #[test]
    fn test_command_generate_json_packet() {
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

    #[test]
    fn test_response_from_json() {
        let ok_response = Response::from_str("{\"id\":1,\"result\":[\"on\"]}").unwrap();
        assert_eq!(ok_response.id, 1);
        assert_eq!(ok_response.result, ResponseResult::Success(vec!("on".to_string())));

        let error_response = "{\"id\":2, \"error\":{\"code\":-1, \"message\":\"unsupported method\"}}";
        let error_response = Response::from_str(error_response).unwrap();

        assert_eq!(error_response.id, 2);
        dbg!(error_response.result);
    }

    #[test]
    fn test_notification_from_json() {
        let notification = "{\"method\":\"props\",\"params\":{\"power\":\"on\", \"bright\": \"10\"}}";
        let notification: Notification = serde_json::from_str(notification).unwrap();

        assert_eq!(notification.method, "props");
        assert_eq!(notification.params.get("power").unwrap(), "on");
        assert_eq!(notification.params.get("bright").unwrap(), "10");
    }
}