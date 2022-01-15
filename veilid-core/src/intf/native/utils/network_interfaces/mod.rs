use crate::xx::*;
use crate::*;
use core::fmt;
mod tools;

cfg_if::cfg_if! {
    if #[cfg(any(target_os = "linux", target_os = "android"))] {
        mod netlink;
        use netlink::PlatformSupportNetlink as PlatformSupport;
    } else if #[cfg(target_os = "windows")] {
        mod windows;
        use windows::PlatformSupportWindows as PlatformSupport;
    } else if #[cfg(any(target_os = "macos", target_os = "ios"))] {
        mod apple;
        mod sockaddr_tools;
        use apple::PlatformSupportApple as PlatformSupport;
    } else {
        compile_error!("No network interfaces support for this platform!");
    }
}

#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Clone)]
pub enum IfAddr {
    V4(Ifv4Addr),
    V6(Ifv6Addr),
}

#[allow(dead_code)]
impl IfAddr {
    pub fn ip(&self) -> IpAddr {
        match *self {
            IfAddr::V4(ref ifv4_addr) => IpAddr::V4(ifv4_addr.ip),
            IfAddr::V6(ref ifv6_addr) => IpAddr::V6(ifv6_addr.ip),
        }
    }
    pub fn netmask(&self) -> IpAddr {
        match *self {
            IfAddr::V4(ref ifv4_addr) => IpAddr::V4(ifv4_addr.netmask),
            IfAddr::V6(ref ifv6_addr) => IpAddr::V6(ifv6_addr.netmask),
        }
    }
    pub fn broadcast(&self) -> Option<IpAddr> {
        match *self {
            IfAddr::V4(ref ifv4_addr) => ifv4_addr.broadcast.map(IpAddr::V4),
            IfAddr::V6(ref ifv6_addr) => ifv6_addr.broadcast.map(IpAddr::V6),
        }
    }
}

/// Details about the ipv4 address of an interface on this host.
#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Clone)]
pub struct Ifv4Addr {
    /// The IP address of the interface.
    pub ip: Ipv4Addr,
    /// The netmask of the interface.
    pub netmask: Ipv4Addr,
    /// The broadcast address of the interface.
    pub broadcast: Option<Ipv4Addr>,
}

/// Details about the ipv6 address of an interface on this host.
#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Clone)]
pub struct Ifv6Addr {
    /// The IP address of the interface.
    pub ip: Ipv6Addr,
    /// The netmask of the interface.
    pub netmask: Ipv6Addr,
    /// The broadcast address of the interface.
    pub broadcast: Option<Ipv6Addr>,
}

/// Some of the flags associated with an interface.
#[derive(Debug, Default, PartialEq, Eq, Ord, PartialOrd, Hash, Clone, Copy)]
pub struct InterfaceFlags {
    pub is_loopback: bool,
    pub is_running: bool,
    pub has_default_route: bool,
}

/// Some of the flags associated with an address.
#[derive(Debug, Default, PartialEq, Eq, Ord, PartialOrd, Hash, Clone, Copy)]
pub struct AddressFlags {
    // common flags
    pub is_dynamic: bool,
    // ipv6 flags
    pub is_temporary: bool,
    pub is_deprecated: bool,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct InterfaceAddress {
    if_addr: IfAddr,
    flags: AddressFlags,
}

use core::cmp::Ordering;

impl Ord for InterfaceAddress {
    fn cmp(&self, other: &Self) -> Ordering {
        match (&self.if_addr, &other.if_addr) {
            (IfAddr::V4(a), IfAddr::V4(b)) => {
                // global scope addresses are better
                let ret = ipv4addr_is_global(&a.ip).cmp(&ipv4addr_is_global(&b.ip));
                if ret != Ordering::Equal {
                    return ret;
                }
                // local scope addresses are better
                let ret = ipv4addr_is_private(&a.ip).cmp(&ipv4addr_is_private(&b.ip));
                if ret != Ordering::Equal {
                    return ret;
                }
                // non-dynamic addresses are better
                let ret = (!self.flags.is_dynamic).cmp(&!other.flags.is_dynamic);
                if ret != Ordering::Equal {
                    return ret;
                }
            }
            (IfAddr::V6(a), IfAddr::V6(b)) => {
                // non-deprecated addresses are better
                let ret = (!self.flags.is_deprecated).cmp(&!other.flags.is_deprecated);
                if ret != Ordering::Equal {
                    return ret;
                }
                // non-temporary address are better
                let ret = (!self.flags.is_temporary).cmp(&!other.flags.is_temporary);
                if ret != Ordering::Equal {
                    return ret;
                }
                // global scope addresses are better
                let ret = ipv6addr_is_global(&a.ip).cmp(&ipv6addr_is_global(&b.ip));
                if ret != Ordering::Equal {
                    return ret;
                }
                // unique local unicast addresses are better
                let ret = ipv6addr_is_unique_local(&a.ip).cmp(&ipv6addr_is_unique_local(&b.ip));
                if ret != Ordering::Equal {
                    return ret;
                }
                // unicast site local addresses are better
                let ret = ipv6addr_is_unicast_site_local(&a.ip)
                    .cmp(&ipv6addr_is_unicast_site_local(&b.ip));
                if ret != Ordering::Equal {
                    return ret;
                }
                // unicast link local addresses are better
                let ret = ipv6addr_is_unicast_link_local(&a.ip)
                    .cmp(&ipv6addr_is_unicast_link_local(&b.ip));
                if ret != Ordering::Equal {
                    return ret;
                }
                // non-dynamic addresses are better
                let ret = (!self.flags.is_dynamic).cmp(&!other.flags.is_dynamic);
                if ret != Ordering::Equal {
                    return ret;
                }
            }
            (IfAddr::V4(_), IfAddr::V6(_)) => return Ordering::Less,
            (IfAddr::V6(_), IfAddr::V4(_)) => return Ordering::Greater,
        }
        // stable sort
        let ret = self.if_addr.cmp(&other.if_addr);
        if ret != Ordering::Equal {
            return ret;
        }
        self.flags.cmp(&other.flags)
    }
}
impl PartialOrd for InterfaceAddress {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[allow(dead_code)]
impl InterfaceAddress {
    pub fn new(if_addr: IfAddr, flags: AddressFlags) -> Self {
        Self { if_addr, flags }
    }

    pub fn if_addr(&self) -> &IfAddr {
        &self.if_addr
    }

    pub fn is_temporary(&self) -> bool {
        self.flags.is_temporary
    }
    pub fn is_dynamic(&self) -> bool {
        self.flags.is_dynamic
    }
    pub fn is_deprecated(&self) -> bool {
        self.flags.is_deprecated
    }
}

#[derive(PartialEq, Eq, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub flags: InterfaceFlags,
    pub addrs: Vec<InterfaceAddress>,
}

impl fmt::Debug for NetworkInterface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetworkInterface")
            .field("name", &self.name)
            .field("flags", &self.flags)
            .field("addrs", &self.addrs)
            .finish()?;
        if f.alternate() {
            writeln!(f)?;
            writeln!(f, "// primary_ipv4: {:?}", self.primary_ipv4())?;
            writeln!(f, "// primary_ipv6: {:?}", self.primary_ipv6())?;
        }
        Ok(())
    }
}
#[allow(dead_code)]
impl NetworkInterface {
    pub fn new(name: String, flags: InterfaceFlags) -> Self {
        Self {
            name,
            flags,
            addrs: Vec::new(),
        }
    }
    pub fn name(&self) -> String {
        self.name.clone()
    }
    pub fn is_loopback(&self) -> bool {
        self.flags.is_loopback
    }

    pub fn is_running(&self) -> bool {
        self.flags.is_running
    }

    pub fn has_default_route(&self) -> bool {
        self.flags.has_default_route
    }

    pub fn primary_ipv4(&self) -> Option<Ipv4Addr> {
        let mut ipv4addrs: Vec<&InterfaceAddress> = self
            .addrs
            .iter()
            .filter(|a| matches!(a.if_addr(), IfAddr::V4(_)))
            .collect();
        ipv4addrs.sort();
        ipv4addrs
            .last()
            .map(|x| match x.if_addr() {
                IfAddr::V4(v4) => Some(v4.ip),
                _ => None,
            })
            .flatten()
    }

    pub fn primary_ipv6(&self) -> Option<Ipv6Addr> {
        let mut ipv6addrs: Vec<&InterfaceAddress> = self
            .addrs
            .iter()
            .filter(|a| matches!(a.if_addr(), IfAddr::V6(_)))
            .collect();
        ipv6addrs.sort();
        ipv6addrs
            .last()
            .map(|x| match x.if_addr() {
                IfAddr::V6(v6) => Some(v6.ip),
                _ => None,
            })
            .flatten()
    }
}

#[derive(PartialEq, Eq, Clone)]
pub struct NetworkInterfaces {
    valid: bool,
    interfaces: BTreeMap<String, NetworkInterface>,
}

impl fmt::Debug for NetworkInterfaces {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetworkInterfaces")
            .field("valid", &self.valid)
            .field("interfaces", &self.interfaces)
            .finish()?;
        if f.alternate() {
            writeln!(f)?;
            writeln!(
                f,
                "// default_route_addresses: {:?}",
                self.default_route_addresses()
            )?;
        }
        Ok(())
    }
}

#[allow(dead_code)]
impl NetworkInterfaces {
    pub fn new() -> Self {
        Self {
            valid: false,
            interfaces: BTreeMap::new(),
        }
    }
    pub fn is_valid(&self) -> bool {
        self.valid
    }
    pub fn clear(&mut self) {
        self.interfaces.clear();
        self.valid = false;
    }
    // returns Ok(false) if refresh had no changes, Ok(true) if changes were present
    pub async fn refresh(&mut self) -> Result<bool, String> {
        self.valid = false;

        let last_interfaces = core::mem::take(&mut self.interfaces);

        let mut platform_support = PlatformSupport::new().map_err(logthru_net!())?;
        platform_support
            .get_interfaces(&mut self.interfaces)
            .await?;

        self.valid = true;

        let changed = last_interfaces != self.interfaces;
        if changed {
            trace!("NetworkInterfaces refreshed: {:#?}?", self);
        }
        Ok(changed)
    }
    pub fn len(&self) -> usize {
        self.interfaces.len()
    }
    pub fn iter(&self) -> std::collections::btree_map::Iter<String, NetworkInterface> {
        self.interfaces.iter()
    }

    pub fn default_route_addresses(&self) -> Vec<IpAddr> {
        let mut out = Vec::new();
        for intf in self.interfaces.values() {
            if intf.is_running() && intf.has_default_route() && !intf.is_loopback() {
                if let Some(pipv4) = intf.primary_ipv4() {
                    out.push(IpAddr::V4(pipv4));
                }
                if let Some(pipv6) = intf.primary_ipv6() {
                    out.push(IpAddr::V6(pipv6));
                }
            }
        }
        out
    }
}
