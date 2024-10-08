use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerInfo {
    routing_domain: RoutingDomain,
    node_ids: TypedKeyGroup,
    signed_node_info: SignedNodeInfo,
}

impl PeerInfo {
    pub fn new(
        routing_domain: RoutingDomain,
        node_ids: TypedKeyGroup,
        signed_node_info: SignedNodeInfo,
    ) -> Self {
        assert!(!node_ids.is_empty() && node_ids.len() <= MAX_CRYPTO_KINDS);
        Self {
            routing_domain,
            node_ids,
            signed_node_info,
        }
    }

    pub fn validate(&self, crypto: Crypto) -> VeilidAPIResult<()> {
        let validated_node_ids = self.signed_node_info.validate(&self.node_ids, crypto)?;
        if validated_node_ids.is_empty() {
            // Shouldn't get here because signed node info validation also checks this
            apibail_generic!("no validated node ids");
        }
        Ok(())
    }

    pub fn routing_domain(&self) -> RoutingDomain {
        self.routing_domain
    }
    pub fn node_ids(&self) -> &TypedKeyGroup {
        &self.node_ids
    }
    pub fn signed_node_info(&self) -> &SignedNodeInfo {
        &self.signed_node_info
    }
    pub fn destructure(self) -> (RoutingDomain, TypedKeyGroup, SignedNodeInfo) {
        (self.routing_domain, self.node_ids, self.signed_node_info)
    }

    pub fn validate_vec(peer_info_vec: &mut Vec<Arc<PeerInfo>>, crypto: Crypto) {
        let mut n = 0usize;
        while n < peer_info_vec.len() {
            let pi = peer_info_vec.get(n).unwrap();
            if pi.validate(crypto.clone()).is_err() {
                peer_info_vec.remove(n);
            } else {
                n += 1;
            }
        }
    }

    /// Compare this PeerInfo to another one
    /// Exclude the signature and timestamp and any other fields that are not
    /// semantically valuable
    /// If the two are not equivalent they should be considered different
    /// enough for republication, but this is not the only criteria required
    /// for publication.
    pub fn equivalent(&self, other: &PeerInfo) -> bool {
        self.routing_domain == other.routing_domain
            && self.node_ids == other.node_ids
            && self.signed_node_info.equivalent(&other.signed_node_info)
    }
}
