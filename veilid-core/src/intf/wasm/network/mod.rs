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
    network_started: bool,
    network_needs_restart: bool,
    protocol_config: Option<ProtocolConfig>,
    //join_handle: TryJoin?
}

#[derive(Clone)]
pub struct Network {
    config: VeilidConfig,
    inner: Arc<Mutex<NetworkInner>>,
}

impl Network {
    fn new_inner(network_manager: NetworkManager) -> NetworkInner {
        NetworkInner {
            network_manager,
            stop_network: Eventual::new(),
            network_started: false,
            network_needs_restart: false,
            protocol_config: None, //join_handle: None,
        }
    }

    pub fn new(network_manager: NetworkManager) -> Self {
        Self {
            config: network_manager.config(),
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
            ProtocolType::UDP => {
                return Err("no support for udp protocol".to_owned()).map_err(logthru_net!(error))
            }
            ProtocolType::TCP => {
                return Err("no support for tcp protocol".to_owned()).map_err(logthru_net!(error))
            }
            ProtocolType::WS | ProtocolType::WSS => {
                // find an existing connection in the connection table if one exists
                let network_manager = self.inner.lock().network_manager.clone();
                if let Some(entry) = network_manager
                    .connection_table()
                    .get_connection(&descriptor)
                {
                    // connection exists, send over it
                    entry.conn.send(data).await.map_err(logthru_net!())?;
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
            DialInfo::UDP(_) => {
                return Err("no support for UDP protocol".to_owned()).map_err(logthru_net!(error))
            }
            DialInfo::TCP(_) => {
                return Err("no support for TCP protocol".to_owned()).map_err(logthru_net!(error))
            }
            DialInfo::WS(_) => Err("WS protocol does not support unbound messages".to_owned())
                .map_err(logthru_net!(error)),
            DialInfo::WSS(_) => Err("WSS protocol does not support unbound messages".to_owned())
                .map_err(logthru_net!(error)),
        }
    }
    pub async fn send_data_to_dial_info(
        &self,
        dial_info: &DialInfo,
        data: Vec<u8>,
    ) -> Result<(), String> {
        let network_manager = self.inner.lock().network_manager.clone();

        let conn = match &dial_info {
            DialInfo::UDP(_) => {
                return Err("no support for UDP protocol".to_owned()).map_err(logthru_net!(error))
            }
            DialInfo::TCP(_) => {
                return Err("no support for TCP protocol".to_owned()).map_err(logthru_net!(error))
            }
            DialInfo::WS(_) => WebsocketProtocolHandler::connect(network_manager, dial_info)
                .await
                .map_err(logthru_net!(error))?,
            DialInfo::WSS(_) => WebsocketProtocolHandler::connect(network_manager, dial_info)
                .await
                .map_err(logthru_net!(error))?,
        };

        conn.send(data).await.map_err(logthru_net!(error))
    }

    pub async fn send_data(&self, node_ref: NodeRef, data: Vec<u8>) -> Result<(), String> {
        let dial_info = node_ref.best_dial_info();
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
            self.clone()
                .send_data_to_dial_info(&di, di_data)
                .await
                .map_err(logthru_net!(error))
        } else {
            Err("couldn't send data, no dial info or peer address".to_owned())
                .map_err(logthru_net!(error))
        }
    }

    /////////////////////////////////////////////////////////////////

    pub async fn startup(&self) -> Result<(), String> {
        // get protocol config
        self.inner.lock().protocol_config = Some({
            let c = self.config.get();
            ProtocolConfig {
                udp_enabled: false, //c.network.protocol.udp.enabled && c.capabilities.protocol_udp,
                tcp_connect: false, //c.network.protocol.tcp.connect && c.capabilities.protocol_connect_tcp,
                tcp_listen: false, //c.network.protocol.tcp.listen && c.capabilities.protocol_accept_tcp,
                ws_connect: c.network.protocol.ws.connect && c.capabilities.protocol_connect_ws,
                ws_listen: c.network.protocol.ws.listen && c.capabilities.protocol_accept_ws,
                wss_connect: c.network.protocol.wss.connect && c.capabilities.protocol_connect_wss,
                wss_listen: c.network.protocol.wss.listen && c.capabilities.protocol_accept_wss,
            }
        });

        self.inner.lock().network_started = true;
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
        routing_table.clear_dial_info_details();

        // Cancels all async background tasks by dropping join handles
        *self.inner.lock() = Self::new_inner(network_manager);

        trace!("network stopped");
    }

    //////////////////////////////////////////
    pub fn get_network_class(&self) -> Option<NetworkClass> {
        // xxx eventually detect tor browser?
        return if self.inner.lock().network_started {
            Some(NetworkClass::WebApp)
        } else {
            None
        };
    }
    pub fn get_protocol_config(&self) -> Option<ProtocolConfig> {
        self.inner.lock().protocol_config.clone()
    }

    //////////////////////////////////////////
    pub async fn tick(&self) -> Result<(), String> {
        Ok(())
    }
}
