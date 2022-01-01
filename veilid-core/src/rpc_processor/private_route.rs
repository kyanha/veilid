use super::*;

impl RPCProcessor {
    //////////////////////////////////////////////////////////////////////
    pub(super) fn new_stub_private_route<'a, T>(
        &self,
        dest_node_id: key::DHTKey,
        builder: &'a mut ::capnp::message::Builder<T>,
    ) -> Result<veilid_capnp::private_route::Reader<'a>, RPCError>
    where
        T: capnp::message::Allocator + 'a,
    {
        let mut pr = builder.init_root::<veilid_capnp::private_route::Builder>();

        let mut pr_pk = pr.reborrow().init_public_key();
        encode_public_key(&dest_node_id, &mut pr_pk)?;
        pr.set_hop_count(0u8);
        // leave firstHop as null
        Ok(pr.into_reader())
    }

    fn encode_safety_route<'a>(
        &self,
        safety_route: &SafetyRouteSpec,
        private_route: veilid_capnp::private_route::Reader<'a>,
        builder: &'a mut veilid_capnp::safety_route::Builder<'a>,
    ) -> Result<(), RPCError> {
        // Ensure the total hop count isn't too long for our config
        let pr_hopcount = private_route.get_hop_count() as usize;
        let sr_hopcount = safety_route.hops.len();
        let hopcount = 1 + sr_hopcount + pr_hopcount;
        if hopcount > self.inner.lock().max_route_hop_count {
            return Err(rpc_error_internal("hop count too long for route"));
        }

        // Build the safety route
        let mut sr_pk = builder.reborrow().init_public_key();
        encode_public_key(&safety_route.public_key, &mut sr_pk)?;

        builder.set_hop_count(
            u8::try_from(sr_hopcount)
                .map_err(map_error_internal!("hop count too large for safety route"))?,
        );

        // Build all the hops in the safety route
        let mut hops_builder = builder.reborrow().init_hops();
        if sr_hopcount == 0 {
            hops_builder
                .set_private(private_route)
                .map_err(map_error_internal!(
                    "invalid private route while encoding safety route"
                ))?;
        } else {
            // start last blob-to-encrypt data off as private route
            let mut blob_data = {
                let mut pr_message = ::capnp::message::Builder::new_default();
                pr_message
                    .set_root_canonical(private_route)
                    .map_err(map_error_internal!(
                        "invalid private route while encoding safety route"
                    ))?;
                let mut blob_data = builder_to_vec(pr_message)?;
                // append the private route tag so we know how to decode it later
                blob_data.push(1u8);
                blob_data
            };

            // Encode each hop from inside to outside
            // skips the outermost hop since that's entering the
            // safety route and does not include the dialInfo
            // (outer hop is a RouteHopData, not a RouteHop).
            // Each loop mutates 'nonce', and 'blob_data'
            let mut nonce = Crypto::get_random_nonce();
            for h in (1..sr_hopcount).rev() {
                // Get blob to encrypt for next hop
                blob_data = {
                    // RouteHop
                    let mut rh_message = ::capnp::message::Builder::new_default();
                    let mut rh_builder = rh_message.init_root::<veilid_capnp::route_hop::Builder>();
                    let mut di_builder = rh_builder.reborrow().init_dial_info();
                    encode_node_dial_info(&safety_route.hops[h].dial_info, &mut di_builder)?;
                    // RouteHopData
                    let mut rhd_builder = rh_builder.init_next_hop();
                    // Add the nonce
                    let mut rhd_nonce = rhd_builder.reborrow().init_nonce();
                    encode_nonce(&nonce, &mut rhd_nonce);
                    // Encrypt the previous blob ENC(nonce, DH(PKhop,SKsr))
                    let dh_secret = self
                        .crypto
                        .cached_dh(
                            &safety_route.hops[h].dial_info.node_id.key,
                            &safety_route.secret_key,
                        )
                        .map_err(map_error_internal!("dh failed"))?;
                    let enc_msg_data =
                        Crypto::encrypt(blob_data.as_slice(), &nonce, &dh_secret, None)
                            .map_err(map_error_internal!("encryption failed"))?;

                    rhd_builder.set_blob(enc_msg_data.as_slice());
                    let mut blob_data = builder_to_vec(rh_message)?;

                    // append the route hop tag so we know how to decode it later
                    blob_data.push(0u8);
                    blob_data
                };
                // Make another nonce for the next hop
                nonce = Crypto::get_random_nonce();
            }

            // Encode first RouteHopData
            let mut first_rhd_builder = hops_builder.init_data();
            let mut first_rhd_nonce = first_rhd_builder.reborrow().init_nonce();
            encode_nonce(&nonce, &mut first_rhd_nonce);
            let dh_secret = self
                .crypto
                .cached_dh(
                    &safety_route.hops[0].dial_info.node_id.key,
                    &safety_route.secret_key,
                )
                .map_err(map_error_internal!("dh failed"))?;
            let enc_msg_data = Crypto::encrypt(blob_data.as_slice(), &nonce, &dh_secret, None)
                .map_err(map_error_internal!("encryption failed"))?;

            first_rhd_builder.set_blob(enc_msg_data.as_slice());
        }

        Ok(())
    }

    // Wrap an operation inside a route
    pub(super) fn wrap_with_route<'a>(
        &self,
        safety_route: Option<&SafetyRouteSpec>,
        private_route: veilid_capnp::private_route::Reader<'a>,
        message_data: Vec<u8>,
    ) -> Result<Vec<u8>, RPCError> {
        // Get stuff before we lock inner
        let op_id = self.get_next_op_id();
        // Encrypt routed operation
        let nonce = Crypto::get_random_nonce();
        let pr_pk_reader = private_route
            .get_public_key()
            .map_err(map_error_internal!("public key is invalid"))?;
        let pr_pk = decode_public_key(&pr_pk_reader);
        let stub_safety_route = SafetyRouteSpec::new();
        let sr = safety_route.unwrap_or(&stub_safety_route);
        let dh_secret = self
            .crypto
            .cached_dh(&pr_pk, &sr.secret_key)
            .map_err(map_error_internal!("dh failed"))?;
        let enc_msg_data = Crypto::encrypt(&message_data, &nonce, &dh_secret, None)
            .map_err(map_error_internal!("encryption failed"))?;

        // Prepare route operation
        let route_msg = {
            let mut route_msg = ::capnp::message::Builder::new_default();
            let mut route_operation = route_msg.init_root::<veilid_capnp::operation::Builder>();

            // Doesn't matter what this op id because there's no answer
            // but it shouldn't conflict with any other op id either
            route_operation.set_op_id(op_id);

            // Answers don't get a 'respond'
            let mut respond_to = route_operation.reborrow().init_respond_to();
            respond_to.set_none(());

            // Set up 'route' operation
            let mut route = route_operation.reborrow().init_detail().init_route();

            // Set the safety route we've constructed
            let mut msg_sr = route.reborrow().init_safety_route();
            self.encode_safety_route(sr, private_route, &mut msg_sr)?;

            // Put in the encrypted operation we're routing
            let mut msg_operation = route.init_operation();
            msg_operation.reborrow().init_signatures(0);
            let mut route_nonce = msg_operation.reborrow().init_nonce();
            encode_nonce(&nonce, &mut route_nonce);
            let data = msg_operation.reborrow().init_data(
                enc_msg_data
                    .len()
                    .try_into()
                    .map_err(map_error_internal!("data too large"))?,
            );
            data.copy_from_slice(enc_msg_data.as_slice());

            route_msg
        };

        // Convert message to bytes and return it
        let out = builder_to_vec(route_msg)?;
        Ok(out)
    }
}
