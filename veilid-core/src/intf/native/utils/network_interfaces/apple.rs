use super::*;

pub struct PlatformSupportApple {
    //
}

impl PlatformSupportApple {
    pub fn new() -> Result<Self, String> {
        Ok(PlatformSupportApple {})
    }

    pub async fn get_interfaces(
        &mut self,
        interfaces: &mut BTreeMap<String, NetworkInterface>,
    ) -> Result<(), String> {
        //
        Ok(())
    }
}
