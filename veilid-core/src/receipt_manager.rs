use crate::*;
use core::fmt;
use dht::*;
use futures_util::stream::{FuturesUnordered, StreamExt};
use network_manager::*;
use routing_table::*;
use xx::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReceiptEvent {
    ReturnedOutOfBand,
    ReturnedInBand { inbound_noderef: NodeRef },
    Expired,
    Cancelled,
}

cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        pub trait ReceiptCallback: 'static {
            fn call(
                &self,
                event: ReceiptEvent,
                receipt: Receipt,
                returns_so_far: u32,
                expected_returns: u32,
            ) -> SystemPinBoxFuture<()>;
        }
        impl<T, F> ReceiptCallback for T
        where
            T: Fn(ReceiptEvent, Receipt, u32, u32) -> F + 'static,
            F: Future<Output = ()> + 'static,
        {
            fn call(
                &self,
                event: ReceiptEvent,
                receipt: Receipt,
                returns_so_far: u32,
                expected_returns: u32,
            ) -> SystemPinBoxFuture<()> {
                Box::pin(self(event, receipt, returns_so_far, expected_returns))
            }
        }
    } else {
        pub trait ReceiptCallback: Send + 'static {
            fn call(
                &self,
                event: ReceiptEvent,
                receipt: Receipt,
                returns_so_far: u32,
                expected_returns: u32,
            ) -> SystemPinBoxFuture<()>;
        }
        impl<F, T> ReceiptCallback for T
        where
            T: Fn(ReceiptEvent, Receipt, u32, u32) -> F + Send + 'static,
            F: Future<Output = ()> + Send + 'static
        {
            fn call(
                &self,
                event: ReceiptEvent,
                receipt: Receipt,
                returns_so_far: u32,
                expected_returns: u32,
            ) -> SystemPinBoxFuture<()> {
                Box::pin(self(event, receipt, returns_so_far, expected_returns))
            }
        }
    }
}

type ReceiptCallbackType = Box<dyn ReceiptCallback>;
type ReceiptSingleShotType = SingleShotEventual<ReceiptEvent>;

enum ReceiptRecordCallbackType {
    Normal(ReceiptCallbackType),
    SingleShot(Option<ReceiptSingleShotType>),
}
impl fmt::Debug for ReceiptRecordCallbackType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ReceiptRecordCallbackType::{}",
            match self {
                Self::Normal(_) => "Normal".to_owned(),
                Self::SingleShot(_) => "SingleShot".to_owned(),
            }
        )
    }
}

pub struct ReceiptRecord {
    expiration_ts: u64,
    receipt: Receipt,
    expected_returns: u32,
    returns_so_far: u32,
    receipt_callback: ReceiptRecordCallbackType,
}

impl fmt::Debug for ReceiptRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReceiptRecord")
            .field("expiration_ts", &self.expiration_ts)
            .field("receipt", &self.receipt)
            .field("expected_returns", &self.expected_returns)
            .field("returns_so_far", &self.returns_so_far)
            .field("receipt_callback", &self.receipt_callback)
            .finish()
    }
}

impl ReceiptRecord {
    pub fn new(
        receipt: Receipt,
        expiration_ts: u64,
        expected_returns: u32,
        receipt_callback: impl ReceiptCallback,
    ) -> Self {
        Self {
            expiration_ts,
            receipt,
            expected_returns,
            returns_so_far: 0u32,
            receipt_callback: ReceiptRecordCallbackType::Normal(Box::new(receipt_callback)),
        }
    }

    pub fn new_single_shot(
        receipt: Receipt,
        expiration_ts: u64,
        eventual: ReceiptSingleShotType,
    ) -> Self {
        Self {
            expiration_ts,
            receipt,
            returns_so_far: 0u32,
            expected_returns: 1u32,
            receipt_callback: ReceiptRecordCallbackType::SingleShot(Some(eventual)),
        }
    }
}

/* XXX: may be useful for O(1) timestamp expiration
#[derive(Clone, Debug)]
struct ReceiptRecordTimestampSort {
    expiration_ts: u64,
    record: Arc<Mutex<ReceiptRecord>>,
}

impl PartialEq for ReceiptRecordTimestampSort {
    fn eq(&self, other: &ReceiptRecordTimestampSort) -> bool {
        self.expiration_ts == other.expiration_ts
    }
}
impl Eq for ReceiptRecordTimestampSort {}
impl Ord for ReceiptRecordTimestampSort {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.expiration_ts.cmp(&other.expiration_ts).reverse()
    }
}
impl PartialOrd for ReceiptRecordTimestampSort {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(&other))
    }
}
*/

///////////////////////////////////

pub struct ReceiptManagerInner {
    network_manager: NetworkManager,
    records_by_nonce: BTreeMap<ReceiptNonce, Arc<Mutex<ReceiptRecord>>>,
    next_oldest_ts: Option<u64>,
    timeout_task: SingleFuture<()>,
}

#[derive(Clone)]
pub struct ReceiptManager {
    inner: Arc<Mutex<ReceiptManagerInner>>,
}

impl ReceiptManager {
    fn new_inner(network_manager: NetworkManager) -> ReceiptManagerInner {
        ReceiptManagerInner {
            network_manager,
            records_by_nonce: BTreeMap::new(),
            next_oldest_ts: None,
            timeout_task: SingleFuture::new(),
        }
    }

    pub fn new(network_manager: NetworkManager) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Self::new_inner(network_manager))),
        }
    }

    pub fn network_manager(&self) -> NetworkManager {
        self.inner.lock().network_manager.clone()
    }

    pub async fn startup(&self) -> Result<(), String> {
        trace!("startup receipt manager");
        // Retrieve config
        /*
                {
                    let config = self.core().config();
                    let c = config.get();
                    let mut inner = self.inner.lock();
                }
        */
        Ok(())
    }

    fn perform_callback(
        evt: ReceiptEvent,
        record_mut: &mut ReceiptRecord,
    ) -> Option<SystemPinBoxFuture<()>> {
        match &mut record_mut.receipt_callback {
            ReceiptRecordCallbackType::Normal(callback) => Some(callback.call(
                evt,
                record_mut.receipt.clone(),
                record_mut.returns_so_far,
                record_mut.expected_returns,
            )),
            ReceiptRecordCallbackType::SingleShot(eventual) => {
                // resolve this eventual with the receiptevent
                // don't need to wait for the instance to receive it
                // because this can only happen once
                if let Some(eventual) = eventual.take() {
                    eventual.resolve(evt);
                }
                None
            }
        }
    }

    pub async fn timeout_task_routine(self, now: u64) {
        // Go through all receipts and build a list of expired nonces
        let mut new_next_oldest_ts: Option<u64> = None;
        let mut expired_records = Vec::new();
        {
            let mut inner = self.inner.lock();
            let mut expired_nonces = Vec::new();
            for (k, v) in &inner.records_by_nonce {
                let receipt_inner = v.lock();
                if receipt_inner.expiration_ts <= now {
                    // Expire this receipt
                    expired_nonces.push(*k);
                } else if new_next_oldest_ts.is_none()
                    || receipt_inner.expiration_ts < new_next_oldest_ts.unwrap()
                {
                    // Mark the next oldest timestamp we would need to take action on as we go through everything
                    new_next_oldest_ts = Some(receipt_inner.expiration_ts);
                }
            }
            if expired_nonces.is_empty() {
                return;
            }
            // Now remove the expired receipts
            for e in expired_nonces {
                let expired_record = inner.records_by_nonce.remove(&e).expect("key should exist");
                expired_records.push(expired_record);
            }
            // Update the next oldest timestamp
            inner.next_oldest_ts = new_next_oldest_ts;
        }
        let mut callbacks = FuturesUnordered::new();
        for expired_record in expired_records {
            let mut expired_record_mut = expired_record.lock();
            if let Some(callback) =
                Self::perform_callback(ReceiptEvent::Expired, &mut expired_record_mut)
            {
                callbacks.push(callback)
            }
        }

        // Wait on all the multi-call callbacks
        while callbacks.next().await.is_some() {}
    }

    pub async fn tick(&self) -> Result<(), String> {
        let (next_oldest_ts, timeout_task) = {
            let inner = self.inner.lock();
            (inner.next_oldest_ts, inner.timeout_task.clone())
        };
        let now = intf::get_timestamp();
        // If we have at least one timestamp to expire, lets do it
        if let Some(next_oldest_ts) = next_oldest_ts {
            if now >= next_oldest_ts {
                // Single-spawn the timeout task routine
                let _ = timeout_task
                    .single_spawn(self.clone().timeout_task_routine(now))
                    .await;
            }
        }
        Ok(())
    }

    pub async fn shutdown(&self) {
        let network_manager = self.network_manager();
        *self.inner.lock() = Self::new_inner(network_manager);
    }

    pub fn record_receipt(
        &self,
        receipt: Receipt,
        expiration: u64,
        expected_returns: u32,
        callback: impl ReceiptCallback,
    ) {
        let receipt_nonce = receipt.get_nonce();
        log_rpc!(debug "== New Multiple Receipt ({}) {} ", expected_returns, receipt_nonce.encode());
        let record = Arc::new(Mutex::new(ReceiptRecord::new(
            receipt,
            expiration,
            expected_returns,
            callback,
        )));
        let mut inner = self.inner.lock();
        inner.records_by_nonce.insert(receipt_nonce, record);

        Self::update_next_oldest_timestamp(&mut *inner);
    }

    pub fn record_single_shot_receipt(
        &self,
        receipt: Receipt,
        expiration: u64,
        eventual: ReceiptSingleShotType,
    ) {
        let receipt_nonce = receipt.get_nonce();
        log_rpc!(debug "== New SingleShot Receipt {}", receipt_nonce.encode());

        let record = Arc::new(Mutex::new(ReceiptRecord::new_single_shot(
            receipt, expiration, eventual,
        )));
        let mut inner = self.inner.lock();
        inner.records_by_nonce.insert(receipt_nonce, record);

        Self::update_next_oldest_timestamp(&mut *inner);
    }

    fn update_next_oldest_timestamp(inner: &mut ReceiptManagerInner) {
        // Update the next oldest timestamp
        let mut new_next_oldest_ts: Option<u64> = None;
        for v in inner.records_by_nonce.values() {
            let receipt_inner = v.lock();
            if new_next_oldest_ts.is_none()
                || receipt_inner.expiration_ts < new_next_oldest_ts.unwrap()
            {
                // Mark the next oldest timestamp we would need to take action on as we go through everything
                new_next_oldest_ts = Some(receipt_inner.expiration_ts);
            }
        }

        inner.next_oldest_ts = new_next_oldest_ts;
    }

    pub async fn cancel_receipt(&self, nonce: &ReceiptNonce) -> Result<(), String> {
        log_rpc!(debug "== Cancel Receipt {}", nonce.encode());

        // Remove the record
        let record = {
            let mut inner = self.inner.lock();
            let record = match inner.records_by_nonce.remove(nonce) {
                Some(r) => r,
                None => {
                    return Err("receipt not recorded".to_owned());
                }
            };
            Self::update_next_oldest_timestamp(&mut *inner);
            record
        };

        // Generate a cancelled callback
        let callback_future = {
            let mut record_mut = record.lock();
            Self::perform_callback(ReceiptEvent::Cancelled, &mut record_mut)
        };

        // Issue the callback
        if let Some(callback_future) = callback_future {
            callback_future.await;
        }

        Ok(())
    }

    pub async fn handle_receipt(
        &self,
        receipt: Receipt,
        inbound_noderef: Option<NodeRef>,
    ) -> Result<(), String> {
        let receipt_nonce = receipt.get_nonce();
        let extra_data = receipt.get_extra_data();

        log_rpc!(debug "<<== RECEIPT {} <- {}{}",
            receipt_nonce.encode(),
            if let Some(nr) = &inbound_noderef {
                nr.to_string()
            } else {
                "DIRECT".to_owned()
            },
            if extra_data.is_empty() {
                "".to_owned()
            } else {
                format!("[{} extra]", extra_data.len())
            }
        );

        // Increment return count
        let callback_future = {
            // Look up the receipt record from the nonce
            let mut inner = self.inner.lock();
            let record = match inner.records_by_nonce.get(&receipt_nonce) {
                Some(r) => r.clone(),
                None => {
                    return Err("receipt not recorded".to_owned());
                }
            };
            // Generate the callback future
            let mut record_mut = record.lock();
            record_mut.returns_so_far += 1;

            // Get the receipt event to return
            let receipt_event = if let Some(inbound_noderef) = inbound_noderef {
                ReceiptEvent::ReturnedInBand { inbound_noderef }
            } else {
                ReceiptEvent::ReturnedOutOfBand
            };

            let callback_future = Self::perform_callback(receipt_event, &mut record_mut);

            // Remove the record if we're done
            if record_mut.returns_so_far == record_mut.expected_returns {
                inner.records_by_nonce.remove(&receipt_nonce);

                Self::update_next_oldest_timestamp(&mut *inner);
            }
            callback_future
        };

        // Issue the callback
        if let Some(callback_future) = callback_future {
            callback_future.await;
        }

        Ok(())
    }
}
