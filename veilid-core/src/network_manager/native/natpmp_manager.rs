use super::*;

pub struct NATPMPManager {
    config: VeilidConfig,
}

impl NATPMPManager {
    //

    pub fn new(config: VeilidConfig) -> Self {
        Self { config }
    }

    pub async fn tick(&self) -> EyreResult<bool> {
        // xxx
        Ok(true)
    }
}
