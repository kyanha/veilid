use crate::*;
pub use wasm_bindgen_futures::*;

#[wasm_bindgen(js_name = VeilidStateChange)]
pub struct JsVeilidStateChange {
    kind: String, // "attachment" => AttachmentState(String)
    from: JsValue,
    to: JsValue,
}
#[wasm_bindgen(js_name = VeilidState)]
pub struct JsVeilidState {
    kind: String, // "attachment" => AttachmentState(String)
    state: JsValue,
}

#[wasm_bindgen(js_name = VeilidCore)]
pub struct JsVeilidCore {
    core: VeilidCore,
}

#[wasm_bindgen(js_class = VeilidCore)]
impl JsVeilidCore {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        set_panic_hook();
        JsVeilidCore {
            core: VeilidCore::new(),
        }
    }
    fn value_to_string(val: JsValue) -> Result<Box<dyn core::any::Any>, ()> {
        Ok(Box::new(val.as_string().ok_or(())?))
    }
    fn value_to_option_string(val: JsValue) -> Result<Box<dyn core::any::Any>, ()> {
        if val.is_null() || val.is_undefined() {
            return Ok(Box::new(Option::<String>::None));
        }
        Ok(Box::new(Some(val.as_string().ok_or(())?)))
    }
    fn value_to_bool(val: JsValue) -> Result<Box<dyn core::any::Any>, ()> {
        Ok(Box::new(val.is_truthy()))
    }
    fn value_to_u8(val: JsValue) -> Result<Box<dyn core::any::Any>, ()> {
        Ok(Box::new(f64_try_to_unsigned::<u8>(
            val.as_f64().ok_or(())?,
        )?))
    }
    fn value_to_u32(val: JsValue) -> Result<Box<dyn core::any::Any>, ()> {
        Ok(Box::new(f64_try_to_unsigned::<u32>(
            val.as_f64().ok_or(())?,
        )?))
    }
    fn value_to_u64(val: JsValue) -> Result<Box<dyn core::any::Any>, ()> {
        Ok(Box::new(f64_try_to_unsigned::<u64>(
            val.as_f64().ok_or(())?,
        )?))
    }
    fn value_to_option_u64(val: JsValue) -> Result<Box<dyn core::any::Any>, ()> {
        if val.is_null() || val.is_undefined() {
            return Ok(Box::new(Option::<u64>::None));
        }

        Ok(Box::new(Some(f64_try_to_unsigned::<u64>(
            val.as_f64().ok_or(())?,
        )?)))
    }
    fn value_to_dht_key(val: JsValue) -> Result<Box<dyn core::any::Any>, ()> {
        Ok(Box::new(
            DHTKey::try_decode(val.as_string().ok_or(())?.as_str()).map_err(drop)?,
        ))
    }
    fn value_to_dht_key_secret(val: JsValue) -> Result<Box<dyn core::any::Any>, ()> {
        Ok(Box::new(
            DHTKeySecret::try_decode(val.as_string().ok_or(())?.as_str()).map_err(drop)?,
        ))
    }
    fn value_to_vec_string(val: JsValue) -> Result<Box<dyn core::any::Any>, ()> {
        let arrval = val.dyn_into::<Array>().map_err(drop)?.to_vec();
        let mut out = Vec::<String>::with_capacity(arrval.len());
        for v in arrval {
            out.push(v.as_string().ok_or(())?);
        }
        Ok(Box::new(out))
    }

    fn translate_config_callback(key: &str, val: JsValue) -> Result<Box<dyn core::any::Any>, ()> {
        match key {
            // xxx: lots of missing keys here
            "namespace" => Self::value_to_string(val),
            "capabilities.protocol_udp" => Self::value_to_bool(val),
            "capabilities.protocol_connect_tcp" => Self::value_to_bool(val),
            "capabilities.protocol_accept_tcp" => Self::value_to_bool(val),
            "capabilities.protocol_connect_ws" => Self::value_to_bool(val),
            "capabilities.protocol_accept_ws" => Self::value_to_bool(val),
            "capabilities.protocol_connect_wss" => Self::value_to_bool(val),
            "capabilities.protocol_accept_wss" => Self::value_to_bool(val),
            "tablestore.directory" => Self::value_to_string(val),
            "network.max_connections" => Self::value_to_u32(val),
            "network.node_id" => Self::value_to_dht_key(val),
            "network.node_id_secret" => Self::value_to_dht_key_secret(val),
            "network.bootstrap" => Self::value_to_vec_string(val),
            "network.rpc.concurrency" => Self::value_to_u32(val),
            "network.rpc.queue_size" => Self::value_to_u32(val),
            "network.rpc.max_timestamp_behind" => Self::value_to_option_u64(val),
            "network.rpc.max_timestamp_ahead" => Self::value_to_option_u64(val),
            "network.rpc.timeout" => Self::value_to_u64(val),
            "network.rpc.max_route_hop_count" => Self::value_to_u8(val),
            "network.dht.resolve_node_timeout" => Self::value_to_option_u64(val),
            "network.dht.resolve_node_count" => Self::value_to_u32(val),
            "network.dht.resolve_node_fanout" => Self::value_to_u32(val),
            "network.dht.max_find_node_count" => Self::value_to_u32(val),
            "network.dht.get_value_timeout" => Self::value_to_option_u64(val),
            "network.dht.get_value_count" => Self::value_to_u32(val),
            "network.dht.get_value_fanout" => Self::value_to_u32(val),
            "network.dht.set_value_timeout" => Self::value_to_option_u64(val),
            "network.dht.set_value_count" => Self::value_to_u32(val),
            "network.dht.set_value_fanout" => Self::value_to_u32(val),
            "network.dht.min_peer_count" => Self::value_to_u32(val),
            "network.dht.min_peer_refresh_time" => Self::value_to_u64(val),
            "network.dht.validate_dial_info_receipt_time" => Self::value_to_u64(val),
            "network.upnp" => Self::value_to_bool(val),
            "network.natpmp" => Self::value_to_bool(val),
            "network.address_filter" => Self::value_to_bool(val),
            "network.restricted_nat_retries" => Self::value_to_u32(val),
            "network.tls.certificate_path" => Self::value_to_string(val),
            "network.tls.private_key_path" => Self::value_to_string(val),
            "network.application.path" => Self::value_to_string(val),
            "network.application.https.enabled" => Self::value_to_bool(val),
            "network.application.https.listen_address" => Self::value_to_string(val),
            "network.application.http.enabled" => Self::value_to_bool(val),
            "network.application.http.listen_address" => Self::value_to_string(val),
            "network.protocol.udp.enabled" => Self::value_to_bool(val),
            "network.protocol.udp.socket_pool_size" => Self::value_to_u32(val),
            "network.protocol.udp.listen_address" => Self::value_to_string(val),
            "network.protocol.udp.public_address" => Self::value_to_option_string(val),
            "network.protocol.tcp.connect" => Self::value_to_bool(val),
            "network.protocol.tcp.listen" => Self::value_to_bool(val),
            "network.protocol.tcp.max_connections" => Self::value_to_u32(val),
            "network.protocol.tcp.listen_address" => Self::value_to_string(val),
            "network.protocol.tcp.public_address" => Self::value_to_option_string(val),
            "network.protocol.ws.connect" => Self::value_to_bool(val),
            "network.protocol.ws.listen" => Self::value_to_bool(val),
            "network.protocol.ws.max_connections" => Self::value_to_u32(val),
            "network.protocol.ws.listen_address" => Self::value_to_string(val),
            "network.protocol.ws.path" => Self::value_to_string(val),
            "network.protocol.ws.public_address" => Self::value_to_option_string(val),
            "network.protocol.wss.connect" => Self::value_to_bool(val),
            "network.protocol.wss.listen" => Self::value_to_bool(val),
            "network.protocol.wss.max_connections" => Self::value_to_u32(val),
            "network.protocol.wss.listen_address" => Self::value_to_string(val),
            "network.protocol.wss.path" => Self::value_to_string(val),
            "network.protocol.wss.public_address" => Self::value_to_option_string(val),
            _ => return Err(()),
        }
    }
    fn translate_veilid_state(state: JsVeilidState) -> Result<VeilidState, JsValue> {
        Ok(match state.kind.as_str() {
            "attachment" => {
                let state_string = state
                    .state
                    .as_string()
                    .ok_or(JsValue::from_str("state should be a string"))?;
                let astate = AttachmentState::try_from(state_string)
                    .map_err(|e| JsValue::from_str(format!("invalid state: {:?}", e).as_str()))?;
                VeilidState::Attachment(astate)
            }
            _ => return Err(JsValue::from_str("unknown state kind")),
        })
    }
    // xxx rework this for new veilid_api mechanism which should be its own js object now
    pub fn startup(
        &self,
        js_state_change_callback: Function,
        js_config_callback: Function,
    ) -> Promise {
        let core = self.core.clone();
        future_to_promise(async move {
            let vcs = VeilidCoreSetup {
                state_change_callback: Arc::new(
                    move |change: VeilidStateChange| -> SystemPinBoxFuture<()> {
                        let js_state_change_callback = js_state_change_callback.clone();
                        Box::pin(async move {
                            let js_change = match change {
                                VeilidStateChange::Attachment {
                                    old_state,
                                    new_state,
                                } => JsVeilidStateChange {
                                    kind: "attachment".to_owned(),
                                    from: JsValue::from_str(old_state.to_string().as_str()),
                                    to: JsValue::from_str(new_state.to_string().as_str()),
                                },
                            };

                            let ret = match Function::call1(
                                &js_state_change_callback,
                                &JsValue::UNDEFINED,
                                &JsValue::from(js_change),
                            ) {
                                Ok(v) => v,
                                Err(e) => {
                                    error!("calling state change callback failed: {:?}", e);
                                    return;
                                }
                            };
                            let retp: Promise = match ret.dyn_into() {
                                Ok(v) => v,
                                Err(e) => {
                                    error!(
                                        "state change callback did not return a promise: {:?}",
                                        e
                                    );
                                    return;
                                }
                            };
                            match JsFuture::from(retp).await {
                                Ok(_) => (),
                                Err(e) => {
                                    error!("state change callback returned an error: {:?}", e);
                                    return;
                                }
                            };
                        })
                    },
                ),
                config_callback: Arc::new(
                    move |key: String| -> Result<Box<dyn core::any::Any>, String> {
                        let val = Function::call1(
                            &js_config_callback,
                            &JsValue::UNDEFINED,
                            &JsValue::from_str(key.as_str()),
                        )
                        .map_err(|_| {
                            format!("Failed to get config from callback for key '{}'", key)
                        })?;

                        Self::translate_config_callback(key.as_str(), val)
                            .map_err(|_| format!("invalid value type for config key '{}'", key))
                    },
                ),
            };

            match core.startup(vcs).await {
                Ok(_) => Ok(JsValue::UNDEFINED),
                Err(e) => Err(JsValue::from_str(
                    format!("VeilidCore startup() failed: {}", e.to_string()).as_str(),
                )),
            }
        })
    }

    pub fn send_state_update(&self) {
        self.core.send_state_update();
    }

    pub fn shutdown(&self) -> Promise {
        let core = self.core.clone();
        future_to_promise(async move {
            core.shutdown().await;
            Ok(JsValue::UNDEFINED)
        })
    }

    pub fn attach(&self) -> Promise {
        let core = self.core.clone();
        future_to_promise(async move {
            core.attach();
            Ok(JsValue::UNDEFINED)
        })
    }

    pub fn detach(&self) -> Promise {
        let core = self.core.clone();
        future_to_promise(async move {
            core.detach();
            Ok(JsValue::UNDEFINED)
        })
    }

    pub fn wait_for_state(&self, state: JsVeilidState) -> Promise {
        let core = self.core.clone();
        future_to_promise(async move {
            let state = Self::translate_veilid_state(state)?;
            core.wait_for_state(state).await;
            Ok(JsValue::UNDEFINED)
        })
    }
}
