use crate::xx::*;
use crate::*;
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
    pub is_temporary: bool,
    pub is_dynamic: bool,
    pub is_deprecated: bool,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct InterfaceAddress {
    if_addr: IfAddr,
    flags: AddressFlags,
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

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct NetworkInterface {
    pub name: String,
    pub flags: InterfaceFlags,
    pub addrs: Vec<InterfaceAddress>,
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
        // see if we have a non-dynamic address to use first
        let mut best_dynamic: Option<Ipv4Addr> = None;
        for x in self.addrs.iter() {
            if let IfAddr::V4(a) = x.if_addr() {
                if !x.is_dynamic() {
                    return Some(a.ip);
                } else if best_dynamic.is_none() {
                    best_dynamic = Some(a.ip);
                }
            }
        }
        best_dynamic
    }
    pub fn primary_ipv6(&self) -> Option<Ipv6Addr> {
        let mut best_dynamic: Option<Ipv6Addr> = None;
        for x in self.addrs.iter() {
            if let IfAddr::V6(a) = x.if_addr() {
                if x.is_temporary() || x.is_deprecated() {
                    if !x.is_dynamic() {
                        return Some(a.ip);
                    } else if best_dynamic.is_none() {
                        best_dynamic = Some(a.ip);
                    }
                }
            }
        }
        best_dynamic
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct NetworkInterfaces {
    valid: bool,
    interfaces: BTreeMap<String, NetworkInterface>,
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

        Ok(last_interfaces != self.interfaces)
    }
    pub fn len(&self) -> usize {
        self.interfaces.len()
    }
    pub fn iter(&self) -> std::collections::btree_map::Iter<String, NetworkInterface> {
        self.interfaces.iter()
    }
}
