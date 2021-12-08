mod protocol;

use crate::intf::*;
use crate::network_manager::*;
use crate::routing_table::*;
use crate::*;
use protocol::ws::WebsocketProtocolHandler;
pub use protocol::*;

/////////////////////////////////////////////////////////////////

struct NetworkInner {
    network_manager: NetworkManager,
    stop_network: Eventual,
    network_needs_restart: bool,
    //join_handle: TryJoin?
}

#[derive(Clone)]
pub struct Network {
    inner: Arc<Mutex<NetworkInner>>,
}

impl Network {
    fn new_inner(network_manager: NetworkManager) -> NetworkInner {
        NetworkInner {
            network_manager,
            stop_network: Eventual::new(),
            network_needs_restart: false,
            //join_handle: None,
        }
    }

    pub fn new(network_manager: NetworkManager) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Self::new_inner(network_manager))),
        }
    }

    /////////////////////////////////////////////////////////////////

    async fn send_data_to_existing_connection(
        &self,
        descriptor: &ConnectionDescriptor,
        data: Vec<u8>,
    ) -> Result<Option<Vec<u8>>, String> {
        match descriptor.protocol_type() {
            ProtocolType::UDP => return Err("no support for udp protocol".to_owned()),
            ProtocolType::TCP => return Err("no support for tcp protocol".to_owned()),
            ProtocolType::WS | ProtocolType::WSS => {
                // find an existing connection in the connection table if one exists
                let network_manager = self.inner.lock().network_manager.clone();
                if let Some(entry) = network_manager
                    .connection_table()
                    .get_connection(&descriptor)
                {
                    // connection exists, send over it
                    entry
                        .conn
                        .send(data)
                        .await
                        .map_err(|_| "failed to send ws message".to_owned())?;
                    // Data was consumed
                    return Ok(None);
                }
            }
        }
        // connection or local socket didn't exist, we'll need to use dialinfo to create one
        // Pass the data back out so we don't own it any more
        Ok(Some(data))
    }

    pub async fn send_data_unbound_to_dial_info(
        &self,
        dial_info: &DialInfo,
        data: Vec<u8>,
    ) -> Result<(), String> {
        let network_manager = self.inner.lock().network_manager.clone();

        match &dial_info {
            DialInfo::UDP(_) => return Err("no support for UDP protocol".to_owned()),
            DialInfo::TCP(_) => return Err("no support for TCP protocol".to_owned()),
            DialInfo::WS(_) => Err("WS protocol does not support unbound messages".to_owned()),
            DialInfo::WSS(_) => Err("WSS protocol does not support unbound messages".to_owned()),
        }
    }
    pub async fn send_data_to_dial_info(
        &self,
        dial_info: &DialInfo,
        data: Vec<u8>,
    ) -> Result<(), String> {
        let network_manager = self.inner.lock().network_manager.clone();

        let conn = match &dial_info {
            DialInfo::UDP(_) => return Err("no support for UDP protocol".to_owned()),
            DialInfo::TCP(_) => return Err("no support for TCP protocol".to_owned()),
            DialInfo::WS(_) => WebsocketProtocolHandler::connect(network_manager, dial_info)
                .await
                .map_err(|_| "failed to connect to WS dial info".to_owned())?,
            DialInfo::WSS(_) => WebsocketProtocolHandler::connect(network_manager, dial_info)
                .await
                .map_err(|_| "failed to connect to WSS dial info".to_owned())?,
        };

        conn.send(data)
            .await
            .map_err(|_| "failed to send data to dial info".to_owned())
    }

    pub async fn send_data(&self, node_ref: NodeRef, data: Vec<u8>) -> Result<(), String> {
        let dial_info = node_ref.dial_info();
        let descriptor = node_ref.last_connection();

        // First try to send data to the last socket we've seen this peer on
        let di_data = if let Some(descriptor) = descriptor {
            match self
                .clone()
                .send_data_to_existing_connection(&descriptor, data)
                .await?
            {
                None => {
                    return Ok(());
                }
                Some(d) => d,
            }
        } else {
            data
        };

        // If that fails, try to make a connection or reach out to the peer via its dial info
        if let Some(di) = dial_info {
            self.clone().send_data_to_dial_info(&di, di_data).await
        } else {
            Err("couldn't send data, no dial info or peer address".to_owned())
        }
    }

    /////////////////////////////////////////////////////////////////

    pub async fn startup(&self) -> Result<(), String> {
        //let network_manager = self.inner.lock().network_manager.clone();
        //let config_shared = network_manager.core().config();
        //let config = config_shared.get();

        Ok(())
    }

    pub fn needs_restart(&self) -> bool {
        self.inner.lock().network_needs_restart
    }

    pub async fn shutdown(&self) {
        trace!("stopping network");

        // Reset state
        let network_manager = self.inner.lock().network_manager.clone();
        let routing_table = network_manager.routing_table();

        // Drop all dial info
        routing_table.clear_local_dial_info();
        routing_table.clear_global_dial_info();

        // Cancels all async background tasks by dropping join handles
        *self.inner.lock() = Self::new_inner(network_manager);

        trace!("network stopped");
    }

    //////////////////////////////////////////
    pub fn get_network_class(&self) -> NetworkClass {
        // xxx eventually detect tor browser?
        return NetworkClass::WebApp;
    }

    //////////////////////////////////////////
    pub async fn tick(&self) -> Result<(), String> {
        Ok(())
    }
}
