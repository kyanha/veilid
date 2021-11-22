use super::*;

#[derive(Debug, Clone)]
pub struct DialInfoEntry {
    dial_info: DialInfo,
    resolved_address: IpAddr,
}

impl DialInfoEntry {
    pub fn try_new(dial_info: DialInfo) -> Result<Self, String> {
        let addr = match dial_info.resolve() {
            Ok(a) => a,
            Err(_) => return Err("failed to resolve address".to_owned()),
        };
        Ok(Self {
            dial_info: dial_info,
            resolved_address: addr,
        })
    }

    pub fn dial_info(&self) -> &DialInfo {
        &self.dial_info
    }

    pub fn address(&self) -> IpAddr {
        self.resolved_address
    }

    pub fn resolve(&mut self) -> Result<IpAddr, String> {
        let addr = match self.dial_info.resolve() {
            Ok(a) => a,
            Err(_) => return Err("failed to resolve address".to_owned()),
        };
        self.resolved_address = addr;
        Ok(addr)
    }

    pub fn matches_peer_scope(&self, scope: PeerScope) -> bool {
        match scope {
            PeerScope::All => true,
            PeerScope::Public => self.is_public(),
            PeerScope::Private => self.is_private(),
        }
    }

    pub fn is_public(&self) -> bool {
        ipaddr_is_global(&self.resolved_address)
    }
    pub fn is_private(&self) -> bool {
        match self.resolved_address {
            IpAddr::V4(a) => ipv4addr_is_private(&a),
            IpAddr::V6(a) => ipv6addr_is_unicast_site_local(&a),
        }
    }
    pub fn is_valid(&self) -> bool {
        self.is_public() || self.is_private()
    }
    pub fn is_loopback(&self) -> bool {
        ipaddr_is_loopback(&self.resolved_address)
    }
}
