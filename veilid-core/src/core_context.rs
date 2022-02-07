use crate::api_logger::*;
use crate::attachment_manager::*;
use crate::dht::crypto::Crypto;
use crate::intf::*;
use crate::veilid_api::*;
use crate::veilid_config::*;
use crate::xx::*;

cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        pub type UpdateCallback = Arc<dyn Fn(VeilidUpdate) -> SystemPinBoxFuture<()>>;
    } else {
        pub type UpdateCallback = Arc<dyn Fn(VeilidUpdate) -> SystemPinBoxFuture<()> + Send + Sync>;
    }
}

pub struct VeilidCoreSetup {
    pub update_callback: UpdateCallback,
    pub config_callback: ConfigCallback,
}

pub struct VeilidCoreContext {
    pub config: VeilidConfig,
    pub protected_store: ProtectedStore,
    pub table_store: TableStore,
    pub block_store: BlockStore,
    pub crypto: Crypto,
    pub attachment_manager: AttachmentManager,
}

impl VeilidCoreContext {
    async fn new(setup: VeilidCoreSetup) -> Result<VeilidCoreContext, VeilidAPIError> {
        // Start up api logging early if it's in the config
        let api_log_level: VeilidConfigLogLevel =
            *(setup.config_callback)("api_log_level".to_owned())
                .map_err(|e| VeilidAPIError::ParseError {
                    message: "Failed to get api_log_level".to_owned(),
                    value: e,
                })?
                .downcast()
                .map_err(|e| VeilidAPIError::ParseError {
                    message: "Incorrect type for key 'api_log_level'".to_owned(),
                    value: format!("Invalid type: {:?}", e.type_id()),
                })?;
        if api_log_level != VeilidConfigLogLevel::Off {
            ApiLogger::init(
                api_log_level.to_level_filter(),
                setup.update_callback.clone(),
            );
            for ig in crate::DEFAULT_LOG_IGNORE_LIST {
                ApiLogger::add_filter_ignore_str(ig);
            }
        }

        trace!("VeilidCoreContext::new starting");

        cfg_if! {
            if #[cfg(target_os = "android")] {
                if utils::android::ANDROID_GLOBALS.lock().is_none() {
                    error!("Android globals are not set up");
                    return Err("Android globals are not set up".to_owned());
                }
            }
        }

        // Set up config
        trace!("VeilidCoreContext::new init config");
        let mut config = VeilidConfig::new();
        if let Err(e) = config.init(setup.config_callback).await {
            ApiLogger::terminate();
            return Err(VeilidAPIError::Internal(e));
        }

        // Set up protected store
        trace!("VeilidCoreContext::new init protected store");
        let protected_store = ProtectedStore::new(config.clone());
        if let Err(e) = protected_store.init().await {
            config.terminate().await;
            ApiLogger::terminate();
            return Err(VeilidAPIError::Internal(e));
        }

        // Init node id from config now that protected store is set up
        if let Err(e) = config.init_node_id(protected_store.clone()).await {
            protected_store.terminate().await;
            config.terminate().await;
            ApiLogger::terminate();
            return Err(VeilidAPIError::Internal(e));
        }

        // Set up tablestore
        trace!("VeilidCoreContext::new init table store");
        let table_store = TableStore::new(config.clone());
        if let Err(e) = table_store.init().await {
            protected_store.terminate().await;
            config.terminate().await;
            ApiLogger::terminate();
            return Err(VeilidAPIError::Internal(e));
        }

        // Set up crypto
        trace!("VeilidCoreContext::new init crypto");
        let crypto = Crypto::new(config.clone(), table_store.clone());
        if let Err(e) = crypto.init().await {
            table_store.terminate().await;
            protected_store.terminate().await;
            config.terminate().await;
            ApiLogger::terminate();
            return Err(VeilidAPIError::Internal(e));
        }

        // Set up block store
        trace!("VeilidCoreContext::new init block store");
        let block_store = BlockStore::new(config.clone());
        if let Err(e) = block_store.init().await {
            crypto.terminate().await;
            table_store.terminate().await;
            protected_store.terminate().await;
            config.terminate().await;
            ApiLogger::terminate();
            return Err(VeilidAPIError::Internal(e));
        }

        // Set up attachment manager
        trace!("VeilidCoreContext::new init attachment manager");
        let cb = setup.update_callback;
        let attachment_manager =
            AttachmentManager::new(config.clone(), table_store.clone(), crypto.clone());
        if let Err(e) = attachment_manager
            .init(Arc::new(
                move |_old_state: AttachmentState, new_state: AttachmentState| {
                    cb(VeilidUpdate::Attachment(new_state))
                },
            ))
            .await
        {
            block_store.terminate().await;
            crypto.terminate().await;
            table_store.terminate().await;
            protected_store.terminate().await;
            config.terminate().await;
            ApiLogger::terminate();
            return Err(VeilidAPIError::Internal(e));
        }

        Ok(VeilidCoreContext {
            config,
            protected_store,
            table_store,
            block_store,
            crypto,
            attachment_manager,
        })
    }

    async fn shutdown(self) {
        trace!("VeilidCoreContext::terminate_core_context starting");

        self.attachment_manager.terminate().await;
        self.block_store.terminate().await;
        self.crypto.terminate().await;
        self.table_store.terminate().await;
        self.protected_store.terminate().await;
        self.config.terminate().await;

        trace!("VeilidCoreContext::shutdown complete");
        ApiLogger::terminate();
    }
}

/////////////////////////////////////////////////////////////////////////////

static INITIALIZED: AsyncMutex<bool> = AsyncMutex::new(false);

pub async fn api_startup(setup: VeilidCoreSetup) -> Result<VeilidAPI, VeilidAPIError> {
    // See if we have an API started up already
    let mut initialized_lock = INITIALIZED.lock().await;
    if *initialized_lock {
        return Err(VeilidAPIError::AlreadyInitialized);
    }

    // Create core context
    let context = VeilidCoreContext::new(setup).await?;

    // Return an API object around our context
    let veilid_api = VeilidAPI::new(context);

    *initialized_lock = true;

    Ok(veilid_api)
}

pub async fn api_shutdown(context: VeilidCoreContext) {
    let mut initialized_lock = INITIALIZED.lock().await;
    context.shutdown().await;
    *initialized_lock = false;
}
