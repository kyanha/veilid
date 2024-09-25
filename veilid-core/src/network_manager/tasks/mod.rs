pub mod rolling_transfers;

use super::*;

impl NetworkManager {
    pub(crate) fn setup_tasks(&self) {
        // Set rolling transfers tick task
        {
            let this = self.clone();
            self.unlocked_inner
                .rolling_transfers_task
                .set_routine(move |s, l, t| {
                    Box::pin(this.clone().rolling_transfers_task_routine(
                        s,
                        Timestamp::new(l),
                        Timestamp::new(t),
                    ))
                });
        }

        // Set address filter task
        {
            let this = self.clone();
            self.unlocked_inner
                .address_filter_task
                .set_routine(move |s, l, t| {
                    Box::pin(this.address_filter().address_filter_task_routine(
                        s,
                        Timestamp::new(l),
                        Timestamp::new(t),
                    ))
                });
        }
    }

    #[instrument(level = "trace", name = "NetworkManager::tick", skip_all, err)]
    pub async fn tick(&self) -> EyreResult<()> {
        let routing_table = self.routing_table();
        let net = self.net();
        let receipt_manager = self.receipt_manager();

        // Run the rolling transfers task
        self.unlocked_inner.rolling_transfers_task.tick().await?;

        // Run the address filter task
        self.unlocked_inner.address_filter_task.tick().await?;

        // Run the routing table tick
        routing_table.tick().await?;

        // Run the low level network tick
        net.tick().await?;

        // Run the receipt manager tick
        receipt_manager.tick().await?;

        // Purge the client allowlist
        self.purge_client_allowlist();

        Ok(())
    }

    pub(crate) async fn cancel_tasks(&self) {
        log_net!(debug "stopping rolling transfers task");
        if let Err(e) = self.unlocked_inner.rolling_transfers_task.stop().await {
            warn!("rolling_transfers_task not stopped: {}", e);
        }

        log_net!(debug "stopping routing table tasks");
        let routing_table = self.routing_table();
        routing_table.cancel_tasks().await;

        // other tasks will get cancelled via the 'shutdown' mechanism
    }
}
