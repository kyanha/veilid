use super::*;

#[derive(Clone, Debug)]
pub struct ConnectionHandle {
    descriptor: ConnectionDescriptor,
    channel: flume::Sender<Vec<u8>>,
}

impl ConnectionHandle {
    pub(super) fn new(descriptor: ConnectionDescriptor, channel: flume::Sender<Vec<u8>>) -> Self {
        Self {
            descriptor,
            channel,
        }
    }

    pub fn connection_descriptor(&self) -> ConnectionDescriptor {
        self.descriptor.clone()
    }

    pub fn send(&self, message: Vec<u8>) -> Result<(), String> {
        self.channel.send(message).map_err(map_to_string)
    }
    pub async fn send_async(&self, message: Vec<u8>) -> Result<(), String> {
        self.channel
            .send_async(message)
            .await
            .map_err(map_to_string)
    }
}

impl PartialEq for ConnectionHandle {
    fn eq(&self, other: &Self) -> bool {
        self.descriptor == other.descriptor
    }
}

impl Eq for ConnectionHandle {}
