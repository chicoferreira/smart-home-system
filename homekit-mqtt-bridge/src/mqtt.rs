use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use log::warn;
use paho_mqtt::{AsyncClient, Message};
use tokio::sync::{broadcast, oneshot};
use tokio::task::JoinHandle;

struct MqttWrapperInner {
    client: AsyncClient,
    get_channels: HashMap<String, oneshot::Sender<Message>>,
    channels: HashMap<String, broadcast::Sender<Message>>,
}

#[derive(Clone)]
pub struct MqttWrapper {
    client: AsyncClient,
    get_channels: Arc<DashMap<String, oneshot::Sender<Message>>>,
    channels: Arc<DashMap<String, broadcast::Sender<Message>>>,
}

impl MqttWrapper {
    pub fn new(client: AsyncClient) -> MqttWrapper {
        MqttWrapper {
            client,
            get_channels: Arc::new(DashMap::new()),
            channels: Arc::new(DashMap::new()),
        }
    }

    pub fn publish<S, V>(&mut self, topic: S, value: V)
        where
            S: Into<String>,
            V: Into<Vec<u8>> {
        let message = Message::new(topic, value, 1);
        self.client.publish(message);
    }

    pub async fn get(&mut self, get_topic: &str, response_topic: &str) -> Result<String, ()> {
        let (sender, receiver): (oneshot::Sender<Message>, oneshot::Receiver<Message>) = oneshot::channel();
        self.get_channels.insert(response_topic.to_string(), sender);

        self.publish(get_topic, []);

        let response = tokio::time::timeout(std::time::Duration::from_secs(5), receiver).await;

        if let Ok(Ok(message)) = response {
            return Ok(message.payload_str().to_string());
        }

        Err(())
    }


    fn start_reading(&mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            let receiver = self.client.get_stream(10);
            while let Ok(message) = receiver.recv().await {
                if let Some(message) = message {
                    self.handle_message(message)
                }
            }
        })
    }

    fn handle_message(&mut self, message: Message) {
        let topic = message.topic();

        if let Some((_, mut sender)) = self.get_channels.remove(topic) {
            if let Err(_) = sender.send(message) {
                warn!("sender dropped")
            }
            return;
        }

        if let Some(sender) = self.channels.get(topic) {
            if let Err(_) = sender.send(message) {
                warn!("sender dropped")
            }
            return;
        }
    }
}
