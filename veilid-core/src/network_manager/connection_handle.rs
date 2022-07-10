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

    pub fn send(&self, message: Vec<u8>) -> EyreResult<()> {
        self.channel
            .send(message)
            .wrap_err("failed to send to connection")
    }
    pub async fn send_async(&self, message: Vec<u8>) -> EyreResult<()> {
        self.channel
            .send_async(message)
            .await
            .wrap_err("failed to send_async to connection")
    }
}

impl PartialEq for ConnectionHandle {
    fn eq(&self, other: &Self) -> bool {
        self.descriptor == other.descriptor
    }
}

impl Eq for ConnectionHandle {}
