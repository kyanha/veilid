#![allow(dead_code)]
use super::*;
use crate::xx::*;
use hex::FromHex;
use if_addrs::{IfAddr, Ifv4Addr, Ifv6Addr};
use std::fs::File;
use std::io::{BufRead, BufReader, Error, ErrorKind};
use std::net::{Ipv4Addr, Ipv6Addr};

#[derive(Debug)]
struct ProcNetIpv6RouteEntry {
    dest_network: Ipv6Addr,
    dest_prefix: u8,
    src_network: Ipv6Addr,
    src_prefix: u8,
    next_hop: Ipv6Addr,
    metric: u32,
    ref_count: u32,
    use_count: u32,
    flags: u32,
    intf_name: String,
}

#[derive(Debug)]
struct ProcNetRouteEntry {
    iface: String,
    destination: Ipv4Addr,
    gateway: Ipv4Addr,
    flags: u16,
    ref_count: u32,
    use_count: u32,
    metric: u32,
    mask: Ipv4Addr,
    mtu: u32,
    window: u32,
    irtt: u32,
}

#[derive(Debug)]
pub struct PlatformSupport {
    proc_net_ipv6_route: Vec<ProcNetIpv6RouteEntry>,
    proc_net_route: Vec<ProcNetRouteEntry>,
}

impl PlatformSupport {
    fn parse_proc_net_ipv6_route() -> Result<Vec<ProcNetIpv6RouteEntry>, Error> {
        let file = File::open("/proc/net/ipv6_route")?;
        let reader = BufReader::new(file);
        let mut ipv6_route: Vec<ProcNetIpv6RouteEntry> = Vec::new();
        for line in reader.lines() {
            let line = line?;
            let line: Vec<&str> = line.split_ascii_whitespace().collect();
            if line.len() != 10 {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "Unexpected number of columns in /proc/net/ipv6_route",
                ));
            }

            let entry =
                ProcNetIpv6RouteEntry {
                    dest_network: Ipv6Addr::from(<[u8; 16]>::from_hex(line[0]).map_err(|_| {
                        Error::new(ErrorKind::InvalidData, "Unable to parse dest_network")
                    })?),
                    dest_prefix: <[u8; 1]>::from_hex(line[1]).map_err(|_| {
                        Error::new(ErrorKind::InvalidData, "Unable to parse dest_prefix")
                    })?[0],
                    src_network: Ipv6Addr::from(<[u8; 16]>::from_hex(line[2]).map_err(|_| {
                        Error::new(ErrorKind::InvalidData, "Unable to parse src_network")
                    })?),
                    src_prefix: <[u8; 1]>::from_hex(line[3]).map_err(|_| {
                        Error::new(ErrorKind::InvalidData, "Unable to parse src_prefix")
                    })?[0],
                    next_hop: Ipv6Addr::from(<[u8; 16]>::from_hex(line[4]).map_err(|_| {
                        Error::new(ErrorKind::InvalidData, "Unable to parse next_hop")
                    })?),
                    metric: u32::from_be_bytes(<[u8; 4]>::from_hex(line[5]).map_err(|_| {
                        Error::new(ErrorKind::InvalidData, "Unable to parse metric")
                    })?),
                    ref_count: u32::from_be_bytes(<[u8; 4]>::from_hex(line[6]).map_err(|_| {
                        Error::new(ErrorKind::InvalidData, "Unable to parse ref_count")
                    })?),
                    use_count: u32::from_be_bytes(<[u8; 4]>::from_hex(line[7]).map_err(|_| {
                        Error::new(ErrorKind::InvalidData, "Unable to parse use_count")
                    })?),
                    flags: u32::from_be_bytes(<[u8; 4]>::from_hex(line[8]).map_err(|_| {
                        Error::new(ErrorKind::InvalidData, "Unable to parse flags")
                    })?),
                    intf_name: String::from(line[9]),
                };

            ipv6_route.push(entry)
        }

        Ok(ipv6_route)
    }

    fn parse_proc_net_route() -> Result<Vec<ProcNetRouteEntry>, Error> {
        let file = File::open("/proc/net/route")?;
        let reader = BufReader::new(file);
        let mut route: Vec<ProcNetRouteEntry> = Vec::new();
        let mut first = false;
        for line in reader.lines() {
            let line = line?;
            let line: Vec<&str> = line.split_ascii_whitespace().collect();
            if line.len() != 11 {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "Unexpected number of columns in /proc/net/route",
                ));
            }
            if first {
                if line
                    != [
                        "Iface",
                        "Destination",
                        "Gateway",
                        "Flags",
                        "RefCnt",
                        "Use",
                        "Metric",
                        "Mask",
                        "MTU",
                        "Window",
                        "IRTT",
                    ]
                {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        format!("Unexpected columns in /proc/net/route: {:?}", line),
                    ));
                }
                first = false;
                continue;
            }

            let entry =
                ProcNetRouteEntry {
                    iface: String::from(line[0]),

                    destination: Ipv4Addr::from(u32::from_le_bytes(
                        <[u8; 4]>::from_hex(line[0]).map_err(|_| {
                            Error::new(ErrorKind::InvalidData, "Unable to parse destination")
                        })?,
                    )),
                    gateway: Ipv4Addr::from(u32::from_le_bytes(
                        <[u8; 4]>::from_hex(line[0]).map_err(|_| {
                            Error::new(ErrorKind::InvalidData, "Unable to parse gateway")
                        })?,
                    )),
                    flags: u16::from_be_bytes(<[u8; 2]>::from_hex(line[8]).map_err(|_| {
                        Error::new(ErrorKind::InvalidData, "Unable to parse flags")
                    })?),
                    ref_count: u32::from_be_bytes(<[u8; 4]>::from_hex(line[6]).map_err(|_| {
                        Error::new(ErrorKind::InvalidData, "Unable to parse ref_count")
                    })?),
                    use_count: u32::from_be_bytes(<[u8; 4]>::from_hex(line[7]).map_err(|_| {
                        Error::new(ErrorKind::InvalidData, "Unable to parse use_count")
                    })?),
                    metric: u32::from_be_bytes(<[u8; 4]>::from_hex(line[5]).map_err(|_| {
                        Error::new(ErrorKind::InvalidData, "Unable to parse metric")
                    })?),
                    mask: Ipv4Addr::from(u32::from_le_bytes(
                        <[u8; 4]>::from_hex(line[0]).map_err(|_| {
                            Error::new(ErrorKind::InvalidData, "Unable to parse mask")
                        })?,
                    )),
                    mtu: u32::from_be_bytes(
                        <[u8; 4]>::from_hex(line[5]).map_err(|_| {
                            Error::new(ErrorKind::InvalidData, "Unable to parse mtu")
                        })?,
                    ),
                    window: u32::from_be_bytes(<[u8; 4]>::from_hex(line[5]).map_err(|_| {
                        Error::new(ErrorKind::InvalidData, "Unable to parse window")
                    })?),
                    irtt: u32::from_be_bytes(
                        <[u8; 4]>::from_hex(line[5]).map_err(|_| {
                            Error::new(ErrorKind::InvalidData, "Unable to parse irtt")
                        })?,
                    ),
                };

            route.push(entry)
        }

        Ok(route)
    }

    pub fn new() -> Result<Self, Error> {
        // Read /proc/net/ipv6_route
        let proc_net_ipv6_route = Self::parse_proc_net_ipv6_route().unwrap_or_default();
        // Read /proc/net/route
        let proc_net_route = Self::parse_proc_net_route().unwrap_or_default();

        trace!("proc_net_ipv6_route: {:#?}", proc_net_ipv6_route);
        trace!("proc_net_route: {:#?}", proc_net_route);

        // At least one routing table must be available
        if proc_net_ipv6_route.is_empty() && proc_net_route.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "No routing tables available",
            ));
        }
        Ok(Self {
            proc_net_ipv6_route,
            proc_net_route,
        })
    }

    pub fn has_default_route(&self, name: &str) -> bool {
        for e in &self.proc_net_ipv6_route {
            if e.intf_name == name && e.dest_prefix == 0u8 {
                return true;
            }
        }
        for e in &self.proc_net_route {
            if e.iface == name && e.mask == Ipv4Addr::new(0, 0, 0, 0) {
                return true;
            }
        }
        false
    }

    pub fn get_address_flags(&self, _addr: &IfAddr) -> AddressFlags {
        AddressFlags {
            is_temporary: false,
            is_dynamic: false,
            is_deprecated: false,
        }
    }
}
