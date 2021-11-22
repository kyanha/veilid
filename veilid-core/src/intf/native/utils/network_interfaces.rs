#[cfg(target_os = "android")]
pub use super::android::*;
use crate::xx::*;
#[cfg(not(target_os = "android"))]
pub use if_addrs::*;

#[derive(PartialEq, Eq, Clone)]
pub struct NetworkInterface {
    name: String,
    is_loopback: bool,
    addrs: Vec<IfAddr>,
}

#[allow(dead_code)]
impl NetworkInterface {
    pub fn new(name: String, is_loopback: bool) -> Self {
        Self {
            name: name,
            is_loopback: is_loopback,
            addrs: Vec::new(),
        }
    }
    pub fn name(&self) -> String {
        self.name.clone()
    }
    pub fn is_loopback(&self) -> bool {
        self.is_loopback
    }
    pub fn primary_ipv4(&self) -> Option<Ipv4Addr> {
        for x in self.addrs.iter() {
            match x {
                IfAddr::V4(a) => return Some(a.ip.clone()),
                _ => (),
            };
        }
        None
    }
    pub fn primary_ipv6(&self) -> Option<Ipv6Addr> {
        for x in self.addrs.iter() {
            match x {
                IfAddr::V6(a) => return Some(a.ip.clone()),
                _ => (),
            };
        }
        None
    }
}

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
    pub fn refresh(&mut self) -> Result<bool, String> {
        self.valid = false;

        let last_interfaces = core::mem::take(&mut self.interfaces);

        let mut intfs = match get_if_addrs() {
            Err(e) => {
                return Err(format!("failed to refresh network interfaces: {}", e));
            }
            Ok(v) => v,
        };
        intfs.sort();

        // debug!("{} interfaces found", intfs.len());
        for intf in intfs {
            // trace!("interface {} at {}", &intf.name, &intf.addr.ip());
            let ni = match self.interfaces.get_mut(&intf.name) {
                None => {
                    self.interfaces.insert(
                        intf.name.clone(),
                        NetworkInterface::new(intf.name.clone(), intf.is_loopback()),
                    );
                    self.interfaces.get_mut(&intf.name).unwrap()
                }
                Some(v) => v,
            };

            ni.addrs.push(intf.addr.clone());
        }

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
