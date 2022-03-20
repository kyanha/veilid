mod protocol;

use crate::connection_manager::*;
use crate::network_manager::*;
use crate::routing_table::*;
use crate::intf::*;
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

    fn connection_manager(&self) -> ConnectionManager {
        self.inner.lock().network_manager.connection_manager()
    }

    /////////////////////////////////////////////////////////////////

    pub async fn send_data_unbound_to_dial_info(
        &self,
        dial_info: DialInfo,
        data: Vec<u8>,
    ) -> Result<(), String> {
        let res = match dial_info.protocol_type() {
            ProtocolType::UDP => {
                return Err("no support for UDP protocol".to_owned()).map_err(logthru_net!(error))
            }
            ProtocolType::TCP => {
                return Err("no support for TCP protocol".to_owned()).map_err(logthru_net!(error))
            }
            ProtocolType::WS | ProtocolType::WSS => {
                WebsocketProtocolHandler::send_unbound_message(dial_info, data)
                    .await
                    .map_err(logthru_net!())
            }
        };
        if res.is_ok() {
            // Network accounting
            self.network_manager()
                .stats_packet_sent(dial_info.to_ip_addr(), data_len as u64);
        }
        res
    }

    async fn send_data_to_existing_connection(
        &self,
        descriptor: ConnectionDescriptor,
        data: Vec<u8>,
    ) -> Result<Option<Vec<u8>>, String> {
        match descriptor.protocol_type() {
            ProtocolType::UDP => {
                return Err("no support for udp protocol".to_owned()).map_err(logthru_net!(error))
            }
            ProtocolType::TCP => {
                return Err("no support for tcp protocol".to_owned()).map_err(logthru_net!(error))
            }
            _ => {}
        }
        
        // Handle connection-oriented protocols

        // Try to send to the exact existing connection if one exists
        if let Some(conn) = self.connection_manager().get_connection(descriptor).await {
            // connection exists, send over it
            conn.send(data).await.map_err(logthru_net!())?;

            // Network accounting
            self.network_manager()
                .stats_packet_sent(descriptor.remote.to_socket_addr().ip(), data_len as u64);

            // Data was consumed
            Ok(None)
        } else {
            // Connection or didn't exist
            // Pass the data back out so we don't own it any more
            Ok(Some(data))
        }
    }

    pub async fn send_data_to_dial_info(
        &self,
        dial_info: DialInfo,
        data: Vec<u8>,
    ) -> Result<(), String> {
        if dial_info.protocol_type() == ProtocolType::UDP {
            return Err("no support for UDP protocol".to_owned()).map_err(logthru_net!(error))
        }
        if dial_info.protocol_type() == ProtocolType::TCP {
            return Err("no support for TCP protocol".to_owned()).map_err(logthru_net!(error))
        }

        // Handle connection-oriented protocols
        let conn = self
            .connection_manager()
            .get_or_create_connection(None, dial_info)
            .await?;

        let res = conn.send(data).await.map_err(logthru_net!(error));
        if res.is_ok() {
            // Network accounting
            self.network_manager()
                .stats_packet_sent(dial_info.to_ip_addr(), data_len as u64);
        }
        res
    }

    pub async fn send_data(&self, node_ref: NodeRef, data: Vec<u8>) -> Result<(), String> {
        // First try to send data to the last socket we've seen this peer on
        let data = if let Some(descriptor) = node_ref.last_connection() {
            match self
                .clone()
                .send_data_to_existing_connection(descriptor, data)
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
        let dial_info = node_ref
            .best_dial_info()
            .ok_or_else(|| "couldn't send data, no dial info or peer address".to_owned())?;

        self.send_data_to_dial_info(dial_info, data).await
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
