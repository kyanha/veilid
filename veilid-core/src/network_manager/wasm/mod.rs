mod protocol;

use super::*;
use crate::routing_table::*;
use connection_manager::*;
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


    fn network_manager(&self) -> NetworkManager {
        self.inner.lock().network_manager.clone()
    }
    fn connection_manager(&self) -> ConnectionManager {
        self.inner.lock().network_manager.connection_manager()
    }

    /////////////////////////////////////////////////////////////////

    #[instrument(level="trace", err, skip(self, data), fields(data.len = data.len()))]
    pub async fn send_data_unbound_to_dial_info(
        &self,
        dial_info: DialInfo,
        data: Vec<u8>,
    ) -> Result<(), String> {
        let data_len = data.len();

        let res = match dial_info.protocol_type() {
            ProtocolType::UDP => {
                return Err("no support for UDP protocol".to_owned()).map_err(logthru_net!(error))
            }
            ProtocolType::TCP => {
                return Err("no support for TCP protocol".to_owned()).map_err(logthru_net!(error))
            }
            ProtocolType::WS | ProtocolType::WSS => {
                WebsocketProtocolHandler::send_unbound_message(dial_info.clone(), data)
                    .await
            }
        };
        if res.is_ok() {
            // Network accounting
            self.network_manager()
                .stats_packet_sent(dial_info.to_ip_addr(), data_len as u64);
        }
        res
    }

    #[instrument(level="trace", err, skip(self, data), fields(data.len = data.len()))]
    pub async fn send_data_to_existing_connection(
        &self,
        descriptor: ConnectionDescriptor,
        data: Vec<u8>,
    ) -> Result<Option<Vec<u8>>, String> {
        let data_len = data.len();
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
            conn.send_async(data).await?;

            // Network accounting
            self.network_manager()
                .stats_packet_sent(descriptor.remote().to_socket_addr().ip(), data_len as u64);

            // Data was consumed
            Ok(None)
        } else {
            // Connection or didn't exist
            // Pass the data back out so we don't own it any more
            Ok(Some(data))
        }
    }

    #[instrument(level="trace", err, skip(self, data), fields(data.len = data.len()))]
    pub async fn send_data_to_dial_info(
        &self,
        dial_info: DialInfo,
        data: Vec<u8>,
    ) -> Result<(), String> {
        let data_len = data.len();
        if dial_info.protocol_type() == ProtocolType::UDP {
            return Err("no support for UDP protocol".to_owned()).map_err(logthru_net!(error))
        }
        if dial_info.protocol_type() == ProtocolType::TCP {
            return Err("no support for TCP protocol".to_owned()).map_err(logthru_net!(error))
        }

        // Handle connection-oriented protocols
        let conn = self
            .connection_manager()
            .get_or_create_connection(None, dial_info.clone())
            .await?;

        let res = conn.send_async(data).await.map_err(logthru_net!(error));
        if res.is_ok() {
            // Network accounting
            self.network_manager()
                .stats_packet_sent(dial_info.to_ip_addr(), data_len as u64);
        }
        res
    }

    /////////////////////////////////////////////////////////////////

    pub async fn startup(&self) -> Result<(), String> {
        // get protocol config
        self.inner.lock().protocol_config = Some({
            let c = self.config.get();
            let inbound = ProtocolSet::new();
            let mut outbound = ProtocolSet::new();

            if c.network.protocol.ws.connect && c.capabilities.protocol_connect_ws {
                outbound.insert(ProtocolType::WS);
            }
            if c.network.protocol.wss.connect && c.capabilities.protocol_connect_wss {
                outbound.insert(ProtocolType::WSS);
            }

            ProtocolConfig { inbound, outbound }
        });

        self.inner.lock().network_started = true;
        Ok(())
    }

    pub fn needs_restart(&self) -> bool {
        self.inner.lock().network_needs_restart
    }

    pub fn is_started(&self) -> bool {
        self.inner.lock().network_started
    }
    
    pub fn restart_network(&self) {
        self.inner.lock().network_needs_restart = true;
    }

    pub async fn shutdown(&self) {
        trace!("stopping network");

        // Reset state
        let network_manager = self.inner.lock().network_manager.clone();
        let routing_table = network_manager.routing_table();

        // Drop all dial info
        routing_table.clear_dial_info_details(RoutingDomain::PublicInternet);
        routing_table.clear_dial_info_details(RoutingDomain::LocalNetwork);

        // Cancels all async background tasks by dropping join handles
        *self.inner.lock() = Self::new_inner(network_manager);

        trace!("network stopped");
    }

    pub fn with_interface_addresses<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[IpAddr]) -> R,
    {
        f(&[])
    }

    pub async fn check_interface_addresses(&self) -> Result<bool, String> {
        Ok(false)
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

    pub fn reset_network_class(&self) {
        //let mut inner = self.inner.lock();
        //inner.network_class = None;
    }

    pub fn get_protocol_config(&self) -> Option<ProtocolConfig> {
        self.inner.lock().protocol_config.clone()
    }

    //////////////////////////////////////////
    pub async fn tick(&self) -> Result<(), String> {
        Ok(())
    }
}
