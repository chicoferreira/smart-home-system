use std::time::Duration;

use anyhow::Context;
use paho_mqtt::{AsyncClient, AsyncReceiver, Message};

pub async fn connect_mqtt(
    subscribe_topics: &[&str],
    server_uri: String,
    username: Option<String>,
    password: Option<String>,
) -> anyhow::Result<(AsyncClient, AsyncReceiver<Option<Message>>)> {
    let create_options = paho_mqtt::CreateOptionsBuilder::new()
        .server_uri(server_uri)
        .client_id("yeelight-controller")
        .finalize();

    let mut client = AsyncClient::new(create_options)
        .context("Failed to create mqtt client")?;

    let mut connection_options = paho_mqtt::ConnectOptionsBuilder::new();

    if let Some(username) = username {
        connection_options.user_name(username);
    }

    if let Some(password) = password {
        connection_options.password(password);
    }

    let connection_options = connection_options
        .keep_alive_interval(Duration::from_secs(20))
        .clean_session(true)
        .automatic_reconnect(Duration::from_secs(1), Duration::from_secs(30))
        .finalize();

    let stream = client.get_stream(10);

    client.connect(connection_options).await.context("Failed to connect to mqtt server")?;

    for &topic in subscribe_topics {
        client.subscribe(topic, 1).await.context(format!("Failed to subscribe to topic: {}", topic))?;
    }

    Ok((client, stream))
}