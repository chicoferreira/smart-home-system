use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream};

enum YeelightMethod {
    GetProp(Vec<String>),
    SetBright(u8),
    SetPower(bool),
    Toggle,
}

impl YeelightMethod {
    fn get_method_name(&self) -> String {
        match self {
            YeelightMethod::GetProp(_) => String::from("get_prop"),
            YeelightMethod::SetBright(_) => String::from("set_bright"),
            YeelightMethod::SetPower(_) => String::from("set_power"),
            YeelightMethod::Toggle => String::from("toggle"),
        }
    }

    fn get_params(&self) -> Vec<String> {
        match self {
            YeelightMethod::GetProp(params) => params.clone(),
            YeelightMethod::SetBright(brightness) => vec![brightness.to_string()],
            YeelightMethod::SetPower(power) => vec![if *power { "on" } else { "off" }.to_string()],
            YeelightMethod::Toggle => vec![],
        }
    }

    fn generate_json_packet(&self, id: u8) -> String {
        serde_json::json!({
            "id": id,
            "method": self.get_method_name(),
            "params": self.get_params(),
        }).to_string() + "\r\n"
    }
}

struct YeelightDevice {
    socket: TcpStream,
    current_id: u8,
}

impl YeelightDevice {
    const DEFAULT_PORT: u16 = 55443;

    fn new(hostname: String, port: u16) -> std::io::Result<Self> {
        let socket = TcpStream::connect((hostname, port)).expect("Failed to connect to device");
        Ok(Self { socket, current_id: 1 })
    }

    fn send_method(&mut self, method: YeelightMethod) -> std::io::Result<String> {
        let packet = method.generate_json_packet(self.current_id);
        self.current_id += 1;
        self.send_packet(&packet)
    }

    fn send_packet(&mut self, packet: &str) -> std::io::Result<String> {
        self.socket.write_all(packet.as_bytes())?;
        self.socket.flush()?;
        self.read_response()
    }

    fn read_response(&mut self) -> std::io::Result<String> {
        let mut lines = BufReader::new(&self.socket).lines();
        let response = lines.next().unwrap()?;
        Ok(response)
    }
}

fn main() {
    let ip = std::env::var("YEELIGHT_HOST").expect("No host address provided. Set env YEELIGHT_HOST to the ip of the yeelight device.");

    println!("{}", YeelightDevice::new(ip, YeelightDevice::DEFAULT_PORT)
        .unwrap()
        .send_method(YeelightMethod::Toggle)
        .unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_json_packet() {
        let method = YeelightMethod::GetProp(vec![String::from("power")]);
        let packet = method.generate_json_packet(1);
        assert_eq!(packet, "{\"id\":1,\"method\":\"get_prop\",\"params\":[\"power\"]}\r\n");
    }

    #[test]
    fn test_generate_json_packet_set_power() {
        assert_eq!(YeelightMethod::SetPower(true).generate_json_packet(1),
                   "{\"id\":1,\"method\":\"set_power\",\"params\":[\"on\"]}\r\n");
    }
}
