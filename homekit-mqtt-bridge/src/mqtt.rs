use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use dashmap::DashMap;
use paho_mqtt::{AsyncClient, Message};
use tokio::task::JoinHandle;

type Callback = Box<dyn Fn(&Message) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

#[derive(Clone)]
pub struct MqttWrapper {
    client: AsyncClient,
    callbacks: Arc<DashMap<String, Callback>>,
}

impl MqttWrapper {
    pub fn new(client: AsyncClient) -> MqttWrapper {
        MqttWrapper {
            client,
            callbacks: Arc::new(DashMap::new()),
        }
    }

    pub fn publish<S, V>(&mut self, topic: S, value: V)
        where
            S: Into<String>,
            V: Into<Vec<u8>> {
        let message = Message::new(topic, value, 1);
        self.client.publish(message);
    }

    pub fn subscribe<S>(&mut self, topic: S, callback: Callback)
        where
            S: Into<String> {
        let topic = topic.into();

        self.client.subscribe(topic.clone(), 1);
        self.callbacks.insert(topic.clone(), callback);
    }

    fn start_reading(&self) -> JoinHandle<()> {
        let mut self_clone = self.clone();
        tokio::spawn(async move {
            let receiver = self_clone.client.get_stream(10);
            while let Ok(message) = receiver.recv().await {
                if let Some(message) = message {
                    self_clone.handle_message(message).await;
                }
            }
        })
    }

    async fn handle_message(&mut self, message: Message) {
        let topic = message.topic();

        if let Some(sender) = self.callbacks.get(topic) {
            sender(&message).await;
        }
    }
}
