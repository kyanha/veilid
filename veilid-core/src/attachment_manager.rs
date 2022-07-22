use crate::callback_state_machine::*;
use crate::dht::Crypto;
use crate::network_manager::*;
use crate::routing_table::*;
use crate::xx::*;
use crate::*;
use core::convert::TryFrom;
use core::fmt;
use serde::*;

state_machine! {
    derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)
    pub Attachment(Detached)
//---
    Detached(AttachRequested) => Attaching [StartAttachment],
    Attaching => {
        AttachmentStopped => Detached,
        WeakPeers => AttachedWeak,
        GoodPeers => AttachedGood,
        StrongPeers => AttachedStrong,
        FullPeers => FullyAttached,
        TooManyPeers => OverAttached,
        DetachRequested => Detaching [StopAttachment]
    },
    AttachedWeak => {
        NoPeers => Attaching,
        GoodPeers => AttachedGood,
        StrongPeers => AttachedStrong,
        FullPeers => FullyAttached,
        TooManyPeers => OverAttached,
        DetachRequested => Detaching [StopAttachment]
    },
    AttachedGood => {
        NoPeers => Attaching,
        WeakPeers => AttachedWeak,
        StrongPeers => AttachedStrong,
        FullPeers => FullyAttached,
        TooManyPeers => OverAttached,
        DetachRequested => Detaching [StopAttachment]
    },
    AttachedStrong => {
        NoPeers => Attaching,
        WeakPeers => AttachedWeak,
        GoodPeers => AttachedGood,
        FullPeers => FullyAttached,
        TooManyPeers => OverAttached,
        DetachRequested => Detaching [StopAttachment]
    },
    FullyAttached => {
        NoPeers => Attaching,
        WeakPeers => AttachedWeak,
        GoodPeers => AttachedGood,
        StrongPeers => AttachedStrong,
        TooManyPeers => OverAttached,
        DetachRequested => Detaching [StopAttachment]
    },
    OverAttached => {
        NoPeers => Attaching,
        WeakPeers => AttachedWeak,
        GoodPeers => AttachedGood,
        StrongPeers => AttachedStrong,
        FullPeers => FullyAttached,
        DetachRequested => Detaching [StopAttachment]
    },
    Detaching => {
        AttachmentStopped => Detached,
    },
}

impl fmt::Display for AttachmentState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        let out = match self {
            AttachmentState::Attaching => "attaching".to_owned(),
            AttachmentState::AttachedWeak => "attached_weak".to_owned(),
            AttachmentState::AttachedGood => "attached_good".to_owned(),
            AttachmentState::AttachedStrong => "attached_strong".to_owned(),
            AttachmentState::FullyAttached => "fully_attached".to_owned(),
            AttachmentState::OverAttached => "over_attached".to_owned(),
            AttachmentState::Detaching => "detaching".to_owned(),
            AttachmentState::Detached => "detached".to_owned(),
        };
        write!(f, "{}", out)
    }
}

impl TryFrom<String> for AttachmentState {
    type Error = ();

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Ok(match s.as_str() {
            "attaching" => AttachmentState::Attaching,
            "attached_weak" => AttachmentState::AttachedWeak,
            "attached_good" => AttachmentState::AttachedGood,
            "attached_strong" => AttachmentState::AttachedStrong,
            "fully_attached" => AttachmentState::FullyAttached,
            "over_attached" => AttachmentState::OverAttached,
            "detaching" => AttachmentState::Detaching,
            "detached" => AttachmentState::Detached,
            _ => return Err(()),
        })
    }
}

pub struct AttachmentManagerInner {
    config: VeilidConfig,
    attachment_machine: CallbackStateMachine<Attachment>,
    network_manager: NetworkManager,
    maintain_peers: bool,
    attach_timestamp: Option<u64>,
    update_callback: Option<UpdateCallback>,
    attachment_maintainer_jh: Option<MustJoinHandle<()>>,
}

#[derive(Clone)]
pub struct AttachmentManager {
    inner: Arc<Mutex<AttachmentManagerInner>>,
}

impl AttachmentManager {
    fn new_inner(
        config: VeilidConfig,
        table_store: TableStore,
        crypto: Crypto,
    ) -> AttachmentManagerInner {
        AttachmentManagerInner {
            config: config.clone(),
            attachment_machine: CallbackStateMachine::new(),
            network_manager: NetworkManager::new(config, table_store, crypto),
            maintain_peers: false,
            attach_timestamp: None,
            update_callback: None,
            attachment_maintainer_jh: None,
        }
    }
    pub fn new(config: VeilidConfig, table_store: TableStore, crypto: Crypto) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Self::new_inner(config, table_store, crypto))),
        }
    }

    pub fn config(&self) -> VeilidConfig {
        self.inner.lock().config.clone()
    }

    pub fn network_manager(&self) -> NetworkManager {
        self.inner.lock().network_manager.clone()
    }

    pub fn is_attached(&self) -> bool {
        let s = self.inner.lock().attachment_machine.state();
        !matches!(s, AttachmentState::Detached | AttachmentState::Detaching)
    }
    pub fn is_detached(&self) -> bool {
        let s = self.inner.lock().attachment_machine.state();
        matches!(s, AttachmentState::Detached)
    }

    pub fn get_attach_timestamp(&self) -> Option<u64> {
        self.inner.lock().attach_timestamp
    }

    fn translate_routing_table_health(
        health: RoutingTableHealth,
        config: &VeilidConfigRoutingTable,
    ) -> AttachmentInput {
        if health.reliable_entry_count >= config.limit_over_attached.try_into().unwrap() {
            return AttachmentInput::TooManyPeers;
        }
        if health.reliable_entry_count >= config.limit_fully_attached.try_into().unwrap() {
            return AttachmentInput::FullPeers;
        }
        if health.reliable_entry_count >= config.limit_attached_strong.try_into().unwrap() {
            return AttachmentInput::StrongPeers;
        }
        if health.reliable_entry_count >= config.limit_attached_good.try_into().unwrap() {
            return AttachmentInput::GoodPeers;
        }
        if health.reliable_entry_count >= config.limit_attached_weak.try_into().unwrap()
            || health.unreliable_entry_count >= config.limit_attached_weak.try_into().unwrap()
        {
            return AttachmentInput::WeakPeers;
        }
        AttachmentInput::NoPeers
    }
    fn translate_attachment_state(state: &AttachmentState) -> AttachmentInput {
        match state {
            AttachmentState::OverAttached => AttachmentInput::TooManyPeers,
            AttachmentState::FullyAttached => AttachmentInput::FullPeers,
            AttachmentState::AttachedStrong => AttachmentInput::StrongPeers,
            AttachmentState::AttachedGood => AttachmentInput::GoodPeers,
            AttachmentState::AttachedWeak => AttachmentInput::WeakPeers,
            AttachmentState::Attaching => AttachmentInput::NoPeers,
            _ => panic!("Invalid state"),
        }
    }

    async fn update_attachment(&self) {
        let new_peer_state_input = {
            let inner = self.inner.lock();

            let old_peer_state_input =
                AttachmentManager::translate_attachment_state(&inner.attachment_machine.state());

            // get reliable peer count from routing table
            let routing_table = inner.network_manager.routing_table();
            let health = routing_table.get_routing_table_health();
            let routing_table_config = &inner.config.get().network.routing_table;

            let new_peer_state_input =
                AttachmentManager::translate_routing_table_health(health, routing_table_config);

            if old_peer_state_input == new_peer_state_input {
                None
            } else {
                Some(new_peer_state_input)
            }
        };
        if let Some(next_input) = new_peer_state_input {
            let _ = self.process_input(&next_input).await;
        }
    }

    #[instrument(level = "debug", skip(self))]
    async fn attachment_maintainer(self) {
        debug!("attachment starting");
        let netman = {
            let mut inner = self.inner.lock();
            inner.attach_timestamp = Some(intf::get_timestamp());
            inner.network_manager.clone()
        };

        let mut restart;
        loop {
            restart = false;
            if let Err(err) = netman.startup().await {
                error!("network startup failed: {}", err);
                netman.shutdown().await;
                break;
            }

            debug!("started maintaining peers");
            while self.inner.lock().maintain_peers {
                // tick network manager
                if let Err(err) = netman.tick().await {
                    error!("Error in network manager: {}", err);
                    self.inner.lock().maintain_peers = false;
                    restart = true;
                    break;
                }

                // see if we need to restart the network
                if netman.needs_restart() {
                    info!("Restarting network");
                    restart = true;
                    break;
                }

                self.update_attachment().await;

                // sleep should be at the end in case maintain_peers changes state
                intf::sleep(1000).await;
            }
            debug!("stopped maintaining peers");

            debug!("stopping network");
            netman.shutdown().await;

            if !restart {
                break;
            }

            debug!("completely restarting attachment");
            // chill out for a second first, give network stack time to settle out
            intf::sleep(1000).await;
        }

        trace!("stopping attachment");
        let attachment_machine = self.inner.lock().attachment_machine.clone();
        let _output = attachment_machine
            .consume(&AttachmentInput::AttachmentStopped)
            .await;
        debug!("attachment stopped");
        self.inner.lock().attach_timestamp = None;
    }

    #[instrument(level = "debug", skip_all, err)]
    pub async fn init(&self, update_callback: UpdateCallback) -> EyreResult<()> {
        trace!("init");
        let network_manager = {
            let mut inner = self.inner.lock();
            inner.update_callback = Some(update_callback.clone());
            let update_callback2 = update_callback.clone();
            inner.attachment_machine.set_state_change_callback(Arc::new(
                move |_old_state: AttachmentState, new_state: AttachmentState| {
                    update_callback2(VeilidUpdate::Attachment(VeilidStateAttachment {
                        state: new_state,
                    }))
                },
            ));
            inner.network_manager.clone()
        };

        network_manager.init(update_callback).await?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn terminate(&self) {
        // Ensure we detached
        self.detach().await;
        let network_manager = {
            let inner = self.inner.lock();
            inner.network_manager.clone()
        };
        network_manager.terminate().await;
        let mut inner = self.inner.lock();
        inner.update_callback = None;
    }

    #[instrument(level = "trace", skip(self))]
    fn attach(&self) {
        // Create long-running connection maintenance routine
        let this = self.clone();
        self.inner.lock().maintain_peers = true;
        self.inner.lock().attachment_maintainer_jh =
            Some(intf::spawn(this.attachment_maintainer()));
    }

    #[instrument(level = "trace", skip(self))]
    async fn detach(&self) {
        let attachment_maintainer_jh = self.inner.lock().attachment_maintainer_jh.take();
        if let Some(jh) = attachment_maintainer_jh {
            // Terminate long-running connection maintenance routine
            self.inner.lock().maintain_peers = false;
            jh.await;
        }
    }

    async fn handle_output(&self, output: &AttachmentOutput) {
        match output {
            AttachmentOutput::StartAttachment => self.attach(),
            AttachmentOutput::StopAttachment => self.detach().await,
        }
    }

    async fn process_input(&self, input: &AttachmentInput) -> EyreResult<()> {
        let attachment_machine = self.inner.lock().attachment_machine.clone();
        let output = attachment_machine.consume(input).await;
        match output {
            Err(e) => Err(eyre!(
                "invalid input '{:?}' for state machine in state '{:?}': {:?}",
                input,
                attachment_machine.state(),
                e
            )),
            Ok(v) => {
                if let Some(o) = v {
                    self.handle_output(&o).await;
                }
                Ok(())
            }
        }
    }

    #[instrument(level = "trace", skip(self), err)]
    pub async fn request_attach(&self) -> EyreResult<()> {
        self.process_input(&AttachmentInput::AttachRequested)
            .await
            .map_err(|e| eyre!("Attach request failed: {}", e))
    }

    #[instrument(level = "trace", skip(self), err)]
    pub async fn request_detach(&self) -> EyreResult<()> {
        self.process_input(&AttachmentInput::DetachRequested)
            .await
            .map_err(|e| eyre!("Detach request failed: {}", e))
    }

    pub fn get_state(&self) -> AttachmentState {
        let attachment_machine = self.inner.lock().attachment_machine.clone();
        attachment_machine.state()
    }

    pub fn get_veilid_state(&self) -> VeilidStateAttachment {
        VeilidStateAttachment {
            state: self.get_state(),
        }
    }
}
