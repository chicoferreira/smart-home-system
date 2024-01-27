use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Context;
use local_ip_address::local_ip;
use log::{error, info};
use tokio::net::UdpSocket;

const SOCKET_CAST_ADDR: SocketAddrV4 = SocketAddrV4::new(MULTI_CAST_ADDR, 1982);
const MULTI_CAST_ADDR: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);
const DISCOVERY_MESSAGE: &[u8] = b"M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1982\r\nMAN: \"ssdp:discover\"\r\nST: wifi_bulb\r\n";

#[derive(Debug, PartialEq)]
pub struct DiscoveryResponse {
    pub model: String,
    pub id: String,
    pub location: String,
}

fn parse(response: &[u8]) -> anyhow::Result<DiscoveryResponse> {
    let response = std::str::from_utf8(response)?;
    let mut model = None;
    let mut id = None;
    let mut location = None;

    for line in response.lines() {
        if let Some((key, value)) = line.split_once(": ") {
            match key {
                "model" => model = Some(value.to_string()),
                "id" => id = Some(value.to_string()),
                "Location" => location = Some(value.to_string()),
                _ => {}
            }
        }
    }

    Ok(DiscoveryResponse {
        model: model.context("No model found in response")?,
        id: id.context("No id found in response")?,
        location: location.context("No location found in response")?,
    })
}

pub async fn discover(timeout: Duration) -> anyhow::Result<Vec<DiscoveryResponse>> {
    let my_local_ip = local_ip().unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    let socket = UdpSocket::bind(SocketAddr::new(my_local_ip, 0)).await?;

    socket.send_to(DISCOVERY_MESSAGE, SOCKET_CAST_ADDR).await?;
    info!("Discovering on {} with timeout {timeout:?}", socket.local_addr()?);

    let mut buf = [0; 2048];

    let responses = Arc::new(Mutex::new(Vec::new()));

    let discover = async {
        loop {
            if let Ok(len) = socket.recv(&mut buf).await {
                match parse(&buf[..len]) {
                    Ok(discovery) => {
                        if let Ok(mut responses) = responses.lock() {
                            if responses.contains(&discovery) {
                                continue;
                            }

                            info!("Found yeelight device: {:?}", discovery);
                            responses.push(discovery);
                        }
                    }
                    Err(err) => error!("Failed to parse discovery response: {}", err),
                }
            }
        }
    };

    let _ = tokio::time::timeout(timeout, discover).await;

    Ok(Arc::try_unwrap(responses).unwrap().into_inner().unwrap())
}