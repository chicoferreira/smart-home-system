use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::str::FromStr;

use serde::Serialize;

#[derive(Serialize)]
pub struct Command {
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

pub struct Device {
    socket: TcpStream,
    current_id: u8,
}

impl Device {
    pub const DEFAULT_PORT: u16 = 55443;

    pub fn new(hostname: String, port: u16) -> std::io::Result<Self> {
        let socket = TcpStream::connect((hostname, port))?;
        Ok(Self { socket, current_id: 1 })
    }

    pub fn send_method(&mut self, method: Method) -> std::io::Result<String> {
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


#[cfg(test)]
mod tests {
    use crate::yeelight::{Command, Method, Power};

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