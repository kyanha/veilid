use super::*;

pub struct PlatformSupportWindows {
    //
}

impl PlatformSupportWindows {
    pub fn new() -> Result<Self, String> {
        Ok(PlatformSupportWindows {})
    }

    pub async fn get_interfaces(
        &mut self,
        interfaces: &mut BTreeMap<String, NetworkInterface>,
    ) -> Result<(), String> {
        //
        Ok(())
    }
}
