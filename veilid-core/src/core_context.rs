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

pub struct VeilidCoreContext {
    pub config: VeilidConfig,
    pub protected_store: ProtectedStore,
    pub table_store: TableStore,
    pub block_store: BlockStore,
    pub crypto: Crypto,
    pub attachment_manager: AttachmentManager,
    pub update_callback: UpdateCallback,
}

impl VeilidCoreContext {
    async fn new_with_config_callback(
        update_callback: UpdateCallback,
        config_callback: ConfigCallback,
    ) -> Result<VeilidCoreContext, VeilidAPIError> {
        // Set up config from callback
        trace!("VeilidCoreContext::new_with_config_callback init config");
        let mut config = VeilidConfig::new();
        if let Err(e) = config.init(config_callback).await {
            return Err(VeilidAPIError::Internal { message: e });
        }

        Self::new_common(update_callback, config).await
    }

    async fn new_with_config_json(
        update_callback: UpdateCallback,
        config_json: String,
    ) -> Result<VeilidCoreContext, VeilidAPIError> {
        // Set up config from callback
        trace!("VeilidCoreContext::new_with_config_json init config");
        let mut config = VeilidConfig::new();
        if let Err(e) = config.init_from_json(config_json).await {
            return Err(VeilidAPIError::Internal { message: e });
        }

        Self::new_common(update_callback, config).await
    }

    async fn new_common(
        update_callback: UpdateCallback,
        config: VeilidConfig,
    ) -> Result<VeilidCoreContext, VeilidAPIError> {
        cfg_if! {
            if #[cfg(target_os = "android")] {
                if utils::android::ANDROID_GLOBALS.lock().is_none() {
                    error!("Android globals are not set up");
                    config.terminate().await;
                    return Err("Android globals are not set up".to_owned());
                }
            }
        }

        // Start up api logging
        let api_log_level: VeilidConfigLogLevel = config.get().api_log_level;
        if api_log_level != VeilidConfigLogLevel::Off {
            ApiLogger::init(api_log_level.to_level_filter(), update_callback.clone());
            for ig in crate::DEFAULT_LOG_IGNORE_LIST {
                ApiLogger::add_filter_ignore_str(ig);
            }
            info!("Veilid API logging initialized");
        }

        // Set up protected store
        trace!("VeilidCoreContext::new init protected store");
        let protected_store = ProtectedStore::new(config.clone());
        if let Err(e) = protected_store.init().await {
            config.terminate().await;
            ApiLogger::terminate();
            return Err(VeilidAPIError::Internal { message: e });
        }

        // Init node id from config now that protected store is set up
        if let Err(e) = config.init_node_id(protected_store.clone()).await {
            protected_store.terminate().await;
            config.terminate().await;
            ApiLogger::terminate();
            return Err(VeilidAPIError::Internal { message: e });
        }

        // Set up tablestore
        trace!("VeilidCoreContext::new init table store");
        let table_store = TableStore::new(config.clone());
        if let Err(e) = table_store.init().await {
            protected_store.terminate().await;
            config.terminate().await;
            ApiLogger::terminate();
            return Err(VeilidAPIError::Internal { message: e });
        }

        // Set up crypto
        trace!("VeilidCoreContext::new init crypto");
        let crypto = Crypto::new(config.clone(), table_store.clone());
        if let Err(e) = crypto.init().await {
            table_store.terminate().await;
            protected_store.terminate().await;
            config.terminate().await;
            ApiLogger::terminate();
            return Err(VeilidAPIError::Internal { message: e });
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
            return Err(VeilidAPIError::Internal { message: e });
        }

        // Set up attachment manager
        trace!("VeilidCoreContext::new init attachment manager");
        let update_callback_move = update_callback.clone();
        let attachment_manager =
            AttachmentManager::new(config.clone(), table_store.clone(), crypto.clone());
        if let Err(e) = attachment_manager
            .init(Arc::new(
                move |_old_state: AttachmentState, new_state: AttachmentState| {
                    update_callback_move(VeilidUpdate::Attachment { state: new_state })
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
            return Err(VeilidAPIError::Internal { message: e });
        }

        Ok(VeilidCoreContext {
            config,
            protected_store,
            table_store,
            block_store,
            crypto,
            attachment_manager,
            update_callback,
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

        // send final shutdown update
        (self.update_callback)(VeilidUpdate::Shutdown).await;

        trace!("VeilidCoreContext::shutdown complete");
        ApiLogger::terminate();
    }
}

/////////////////////////////////////////////////////////////////////////////

static INITIALIZED: AsyncMutex<bool> = AsyncMutex::new(false);

pub async fn api_startup(
    update_callback: UpdateCallback,
    config_callback: ConfigCallback,
) -> Result<VeilidAPI, VeilidAPIError> {
    // See if we have an API started up already
    let mut initialized_lock = INITIALIZED.lock().await;
    if *initialized_lock {
        return Err(VeilidAPIError::AlreadyInitialized);
    }

    // Create core context
    let context =
        VeilidCoreContext::new_with_config_callback(update_callback, config_callback).await?;

    // Return an API object around our context
    let veilid_api = VeilidAPI::new(context);

    *initialized_lock = true;

    Ok(veilid_api)
}

pub async fn api_startup_json(
    update_callback: UpdateCallback,
    config_json: String,
) -> Result<VeilidAPI, VeilidAPIError> {
    // See if we have an API started up already
    let mut initialized_lock = INITIALIZED.lock().await;
    if *initialized_lock {
        return Err(VeilidAPIError::AlreadyInitialized);
    }

    // Create core context
    let context = VeilidCoreContext::new_with_config_json(update_callback, config_json).await?;

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
