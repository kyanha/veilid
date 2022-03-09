use crate::callback_state_machine::*;
use crate::dht::crypto::Crypto;
use crate::intf::*;
use crate::network_manager::*;
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
    peer_count: u32,
    attach_timestamp: Option<u64>,
    attachment_maintainer_jh: Option<JoinHandle<()>>,
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
            peer_count: 0,
            attach_timestamp: None,
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

    pub fn get_peer_count(&self) -> u32 {
        self.inner.lock().peer_count
    }

    fn translate_peer_input(cur: u32, max: u32) -> AttachmentInput {
        if cur > max {
            return AttachmentInput::TooManyPeers;
        }
        match cmp::min(4, 4 * cur / max) {
            4 => AttachmentInput::FullPeers,
            3 => AttachmentInput::StrongPeers,
            2 => AttachmentInput::GoodPeers,
            1 => AttachmentInput::WeakPeers,
            0 => AttachmentInput::NoPeers,
            _ => panic!("Invalid state"),
        }
    }
    fn translate_peer_state(state: &AttachmentState) -> AttachmentInput {
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

    async fn update_peer_count(&self) {
        let new_peer_state_input = {
            let inner = self.inner.lock();

            let old_peer_state_input =
                AttachmentManager::translate_peer_state(&inner.attachment_machine.state());

            let max_connections = inner.config.get().network.max_connections;

            // get active peer count from routing table

            let new_peer_state_input =
                AttachmentManager::translate_peer_input(inner.peer_count, max_connections);

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

    async fn attachment_maintainer(self) {
        trace!("attachment starting");
        let netman = {
            let mut inner = self.inner.lock();
            inner.attach_timestamp = Some(intf::get_timestamp());
            inner.network_manager.clone()
        };

        trace!("starting network");
        let mut started = true;
        if let Err(err) = netman.startup().await {
            error!("network startup failed: {}", err);
            started = false;
        }

        if started {
            trace!("started maintaining peers");
            while self.inner.lock().maintain_peers {
                // tick network manager
                if let Err(err) = netman.tick().await {
                    error!("Error in network manager: {}", err);
                    self.inner.lock().maintain_peers = false;
                    break;
                }

                // xxx: ?update peer count?
                self.update_peer_count().await;

                // sleep should be at the end in case maintain_peers changes state
                intf::sleep(1000).await;
            }
            trace!("stopped maintaining peers");

            trace!("stopping network");
            netman.shutdown().await;
        }

        trace!("stopping attachment");
        let attachment_machine = self.inner.lock().attachment_machine.clone();
        let _output = attachment_machine
            .consume(&AttachmentInput::AttachmentStopped)
            .await;
        trace!("attachment stopped");
        self.inner.lock().attach_timestamp = None;
    }

    pub async fn init(
        &self,
        state_change_callback: StateChangeCallback<Attachment>,
    ) -> Result<(), String> {
        trace!("init");
        let network_manager = {
            let inner = self.inner.lock();
            inner
                .attachment_machine
                .set_state_change_callback(state_change_callback);
            inner.network_manager.clone()
        };

        network_manager.init().await?;

        Ok(())
    }
    pub async fn terminate(&self) {
        // Ensure we detached
        self.detach().await;
        let network_manager = {
            let inner = self.inner.lock();
            inner.network_manager.clone()
        };
        network_manager.terminate().await;
    }

    fn attach(&self) {
        trace!("attach");
        // Create long-running connection maintenance routine
        let this = self.clone();
        self.inner.lock().maintain_peers = true;
        self.inner.lock().attachment_maintainer_jh =
            Some(intf::spawn(this.attachment_maintainer()));
    }

    async fn detach(&self) {
        trace!("detach");
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

    async fn process_input(&self, input: &AttachmentInput) -> Result<(), String> {
        let attachment_machine = self.inner.lock().attachment_machine.clone();
        let output = attachment_machine.consume(input).await;
        match output {
            Err(e) => Err(format!(
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

    pub async fn request_attach(&self) -> Result<(), String> {
        self.process_input(&AttachmentInput::AttachRequested)
            .await
            .map_err(|e| format!("Attach request failed: {}", e))
    }

    pub async fn request_detach(&self) -> Result<(), String> {
        self.process_input(&AttachmentInput::DetachRequested)
            .await
            .map_err(|e| format!("Attach request failed: {}", e))
    }

    pub fn get_state(&self) -> AttachmentState {
        let attachment_machine = self.inner.lock().attachment_machine.clone();
        attachment_machine.state()
    }
}
