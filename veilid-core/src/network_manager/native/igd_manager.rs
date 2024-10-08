use super::*;
use igd::*;
use std::net::UdpSocket;

const UPNP_GATEWAY_DETECT_TIMEOUT_MS: u32 = 5_000;
const UPNP_MAPPING_LIFETIME_MS: u32 = 120_000;
const UPNP_MAPPING_ATTEMPTS: u32 = 3;
const UPNP_MAPPING_LIFETIME_US: TimestampDuration =
    TimestampDuration::new(UPNP_MAPPING_LIFETIME_MS as u64 * 1000u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct PortMapKey {
    llpt: LowLevelProtocolType,
    at: AddressType,
    local_port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct PortMapValue {
    ext_ip: IpAddr,
    mapped_port: u16,
    timestamp: Timestamp,
    renewal_lifetime: TimestampDuration,
    renewal_attempts: u32,
}

struct IGDManagerInner {
    local_ip_addrs: BTreeMap<AddressType, IpAddr>,
    gateways: BTreeMap<IpAddr, Arc<Gateway>>,
    port_maps: BTreeMap<PortMapKey, PortMapValue>,
}

#[derive(Clone)]
pub struct IGDManager {
    config: VeilidConfig,
    inner: Arc<Mutex<IGDManagerInner>>,
}

fn convert_llpt(llpt: LowLevelProtocolType) -> PortMappingProtocol {
    match llpt {
        LowLevelProtocolType::UDP => PortMappingProtocol::UDP,
        LowLevelProtocolType::TCP => PortMappingProtocol::TCP,
    }
}

impl IGDManager {
    //

    pub fn new(config: VeilidConfig) -> Self {
        Self {
            config,
            inner: Arc::new(Mutex::new(IGDManagerInner {
                local_ip_addrs: BTreeMap::new(),
                gateways: BTreeMap::new(),
                port_maps: BTreeMap::new(),
            })),
        }
    }

    #[instrument(level = "trace", target = "net", skip_all)]
    fn get_routed_local_ip_address(address_type: AddressType) -> Option<IpAddr> {
        let socket = match UdpSocket::bind(match address_type {
            AddressType::IPV4 => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            AddressType::IPV6 => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        }) {
            Ok(s) => s,
            Err(e) => {
                log_net!(debug "failed to bind to unspecified address: {}", e);
                return None;
            }
        };

        // can be any routable ip address,
        // this is just to make the system routing table calculate the appropriate local ip address
        // using google's dns, but it wont actually send any packets to it
        socket
            .connect(match address_type {
                AddressType::IPV4 => SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 80),
                AddressType::IPV6 => SocketAddr::new(
                    IpAddr::V6(Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888)),
                    80,
                ),
            })
            .map_err(|e| {
                log_net!(debug "failed to connect to dummy address: {}", e);
                e
            })
            .ok()?;

        Some(socket.local_addr().ok()?.ip())
    }

    #[instrument(level = "trace", target = "net", skip_all)]
    fn find_local_ip(inner: &mut IGDManagerInner, address_type: AddressType) -> Option<IpAddr> {
        if let Some(ip) = inner.local_ip_addrs.get(&address_type) {
            return Some(*ip);
        }

        let ip = match Self::get_routed_local_ip_address(address_type) {
            Some(x) => x,
            None => {
                log_net!(debug "failed to get local ip address: address_type={:?}", address_type);
                return None;
            }
        };

        inner.local_ip_addrs.insert(address_type, ip);
        Some(ip)
    }

    #[instrument(level = "trace", target = "net", skip_all)]
    fn get_local_ip(inner: &mut IGDManagerInner, address_type: AddressType) -> Option<IpAddr> {
        if let Some(ip) = inner.local_ip_addrs.get(&address_type) {
            return Some(*ip);
        }
        None
    }

    #[instrument(level = "trace", target = "net", skip_all)]
    fn find_gateway(inner: &mut IGDManagerInner, local_ip: IpAddr) -> Option<Arc<Gateway>> {
        if let Some(gw) = inner.gateways.get(&local_ip) {
            return Some(gw.clone());
        }

        let gateway = match local_ip {
            IpAddr::V4(v4) => {
                let mut opts = SearchOptions::new_v4(UPNP_GATEWAY_DETECT_TIMEOUT_MS as u64);
                opts.bind_addr = SocketAddr::V4(SocketAddrV4::new(v4, 0));

                match igd::search_gateway(opts) {
                    Ok(v) => v,
                    Err(e) => {
                        log_net!(debug "couldn't find ipv4 igd: {}", e);
                        return None;
                    }
                }
            }
            IpAddr::V6(v6) => {
                let mut opts = SearchOptions::new_v6(
                    Ipv6SearchScope::LinkLocal,
                    UPNP_GATEWAY_DETECT_TIMEOUT_MS as u64,
                );
                opts.bind_addr = SocketAddr::V6(SocketAddrV6::new(v6, 0, 0, 0));

                match igd::search_gateway(opts) {
                    Ok(v) => v,
                    Err(e) => {
                        log_net!(debug "couldn't find ipv6 igd: {}", e);
                        return None;
                    }
                }
            }
        };
        let gw = Arc::new(gateway);
        inner.gateways.insert(local_ip, gw.clone());
        Some(gw)
    }

    #[instrument(level = "trace", target = "net", skip_all)]
    fn get_gateway(inner: &mut IGDManagerInner, local_ip: IpAddr) -> Option<Arc<Gateway>> {
        if let Some(gw) = inner.gateways.get(&local_ip) {
            return Some(gw.clone());
        }
        None
    }

    fn get_description(&self, llpt: LowLevelProtocolType, local_port: u16) -> String {
        format!(
            "{} map {} for port {}",
            self.config.get().program_name,
            convert_llpt(llpt),
            local_port
        )
    }

    #[instrument(level = "trace", target = "net", skip_all)]
    pub async fn unmap_port(
        &self,
        llpt: LowLevelProtocolType,
        at: AddressType,
        mapped_port: u16,
    ) -> Option<()> {
        let this = self.clone();
        blocking_wrapper(
            "igd unmap_port",
            move || {
                let mut inner = this.inner.lock();

                // If we already have this port mapped, just return the existing portmap
                let mut found = None;
                for (pmk, pmv) in &inner.port_maps {
                    if pmk.llpt == llpt && pmk.at == at && pmv.mapped_port == mapped_port {
                        found = Some(*pmk);
                        break;
                    }
                }
                let pmk = found?;
                let _pmv = inner
                    .port_maps
                    .remove(&pmk)
                    .expect("key found but remove failed");

                // Get local ip address
                let local_ip = Self::find_local_ip(&mut inner, at)?;

                // Find gateway
                let gw = Self::find_gateway(&mut inner, local_ip)?;

                // Unmap port
                match gw.remove_port(convert_llpt(llpt), mapped_port) {
                    Ok(()) => (),
                    Err(e) => {
                        // Failed to map external port
                        log_net!(debug "upnp failed to remove external port: {}", e);
                        return None;
                    }
                };
                Some(())
            },
            None,
        )
        .await
    }

    #[instrument(level = "trace", target = "net", skip_all)]
    pub async fn map_any_port(
        &self,
        llpt: LowLevelProtocolType,
        at: AddressType,
        local_port: u16,
        expected_external_address: Option<IpAddr>,
    ) -> Option<SocketAddr> {
        let this = self.clone();
        blocking_wrapper("igd map_any_port", move || {
            let mut inner = this.inner.lock();

            // If we already have this port mapped, just return the existing portmap
            let pmkey = PortMapKey {
                llpt,
                at,
                local_port,
            };
            if let Some(pmval) = inner.port_maps.get(&pmkey) {
                return Some(SocketAddr::new(pmval.ext_ip, pmval.mapped_port));
            }

            // Get local ip address
            let local_ip = Self::find_local_ip(&mut inner, at)?;

            // Find gateway
            let gw = Self::find_gateway(&mut inner, local_ip)?;

            // Get external address
            let ext_ip = match gw.get_external_ip() {
                Ok(ip) => ip,
                Err(e) => {
                    log_net!(debug "couldn't get external ip from igd: {}", e);
                    return None;
                }
            };

            // Ensure external IP matches address type
            if ext_ip.is_ipv4() && at != AddressType::IPV4 {
                log_net!(debug "mismatched ip address type from igd, wanted v4, got v6");
                return None;
            } else if ext_ip.is_ipv6() && at != AddressType::IPV6 {
                log_net!(debug "mismatched ip address type from igd, wanted v6, got v4");
                return None;
            }

            if let Some(expected_external_address) = expected_external_address {
                if ext_ip != expected_external_address {
                    log_net!(debug "gateway external address does not match calculated external address: expected={} vs gateway={}", expected_external_address, ext_ip);
                    return None;
                }
            }

            // Map any port
            let desc = this.get_description(llpt, local_port);
            let mapped_port = match gw.add_any_port(convert_llpt(llpt), SocketAddr::new(local_ip, local_port), (UPNP_MAPPING_LIFETIME_MS + 999) / 1000, &desc) {
                Ok(mapped_port) => mapped_port,
                Err(e) => {
                    // Failed to map external port
                    log_net!(debug "upnp failed to map external port: {}", e);
                    return None;
                }
            };

            // Add to mapping list to keep alive
            let timestamp = Timestamp::now();
            inner.port_maps.insert(PortMapKey {
                llpt,
                at,
                local_port,
            }, PortMapValue {
                ext_ip,
                mapped_port,
                timestamp,
                renewal_lifetime: ((UPNP_MAPPING_LIFETIME_MS / 2) as u64 * 1000u64).into(),
                renewal_attempts: 0,
            });

            // Succeeded, return the externally mapped port
            Some(SocketAddr::new(ext_ip, mapped_port))
        }, None)
        .await
    }

    #[instrument(
        level = "trace",
        target = "net",
        name = "IGDManager::tick",
        skip_all,
        err
    )]
    pub async fn tick(&self) -> EyreResult<bool> {
        // Refresh mappings if we have them
        // If an error is received, then return false to restart the local network
        let mut full_renews: Vec<(PortMapKey, PortMapValue)> = Vec::new();
        let mut renews: Vec<(PortMapKey, PortMapValue)> = Vec::new();
        {
            let inner = self.inner.lock();
            let now = Timestamp::now();

            for (k, v) in &inner.port_maps {
                let mapping_lifetime = now.saturating_sub(v.timestamp);
                if mapping_lifetime >= UPNP_MAPPING_LIFETIME_US
                    || v.renewal_attempts >= UPNP_MAPPING_ATTEMPTS
                {
                    // Past expiration time or tried N times, do a full renew and fail out if we can't
                    full_renews.push((*k, *v));
                } else if mapping_lifetime >= v.renewal_lifetime {
                    // Attempt a normal renewal
                    renews.push((*k, *v));
                }
            }

            // See if we need to do some blocking operations
            if full_renews.is_empty() && renews.is_empty() {
                // Just return now since there's nothing to renew
                return Ok(true);
            }
        }

        let this = self.clone();
        blocking_wrapper(
            "igd tick",
            move || {
                let mut inner = this.inner.lock();

                // Process full renewals
                for (k, v) in full_renews {
                    // Get local ip for address type
                    let local_ip = match Self::get_local_ip(&mut inner, k.at) {
                        Some(ip) => ip,
                        None => {
                            return Err(eyre!("local ip missing for address type"));
                        }
                    };

                    // Get gateway for interface
                    let gw = match Self::get_gateway(&mut inner, local_ip) {
                        Some(gw) => gw,
                        None => {
                            return Err(eyre!("gateway missing for interface"));
                        }
                    };

                    // Delete the mapping if it exists, ignore any errors here
                    let _ = gw.remove_port(convert_llpt(k.llpt), v.mapped_port);
                    inner.port_maps.remove(&k);

                    let desc = this.get_description(k.llpt, k.local_port);
                    match gw.add_any_port(
                        convert_llpt(k.llpt),
                        SocketAddr::new(local_ip, k.local_port),
                        (UPNP_MAPPING_LIFETIME_MS + 999) / 1000,
                        &desc,
                    ) {
                        Ok(mapped_port) => {
                            log_net!(debug "full-renewed mapped port {:?} -> {:?}", v, k);
                            inner.port_maps.insert(
                                k,
                                PortMapValue {
                                    ext_ip: v.ext_ip,
                                    mapped_port,
                                    timestamp: Timestamp::now(),
                                    renewal_lifetime: TimestampDuration::new(
                                        (UPNP_MAPPING_LIFETIME_MS / 2) as u64 * 1000u64,
                                    ),
                                    renewal_attempts: 0,
                                },
                            );
                        }
                        Err(e) => {
                            info!("failed to full-renew mapped port {:?} -> {:?}: {}", v, k, e);

                            // Must restart network now :(
                            return Ok(false);
                        }
                    };
                }
                // Process normal renewals
                for (k, mut v) in renews {
                    // Get local ip for address type
                    let local_ip = match Self::get_local_ip(&mut inner, k.at) {
                        Some(ip) => ip,
                        None => {
                            return Err(eyre!("local ip missing for address type"));
                        }
                    };

                    // Get gateway for interface
                    let gw = match Self::get_gateway(&mut inner, local_ip) {
                        Some(gw) => gw,
                        None => {
                            return Err(eyre!("gateway missing for address type"));
                        }
                    };

                    let desc = this.get_description(k.llpt, k.local_port);
                    match gw.add_port(
                        convert_llpt(k.llpt),
                        v.mapped_port,
                        SocketAddr::new(local_ip, k.local_port),
                        (UPNP_MAPPING_LIFETIME_MS + 999) / 1000,
                        &desc,
                    ) {
                        Ok(()) => {
                            log_net!("renewed mapped port {:?} -> {:?}", v, k);

                            inner.port_maps.insert(
                                k,
                                PortMapValue {
                                    ext_ip: v.ext_ip,
                                    mapped_port: v.mapped_port,
                                    timestamp: Timestamp::now(),
                                    renewal_lifetime: ((UPNP_MAPPING_LIFETIME_MS / 2) as u64
                                        * 1000u64)
                                        .into(),
                                    renewal_attempts: 0,
                                },
                            );
                        }
                        Err(e) => {
                            log_net!(debug "failed to renew mapped port {:?} -> {:?}: {}", v, k, e);

                            // Get closer to the maximum renewal timeline by a factor of two each time
                            v.renewal_lifetime =
                                (v.renewal_lifetime + UPNP_MAPPING_LIFETIME_US) / 2u64;
                            v.renewal_attempts += 1;

                            // Store new value to try again
                            inner.port_maps.insert(k, v);
                        }
                    };
                }

                // Normal exit, no restart
                Ok(true)
            },
            Err(eyre!("failed to process blocking task")),
        )
        .instrument(tracing::trace_span!("igd tick fut"))
        .await
    }
}
