use crate::*;

use rand::{CryptoRng, Error, RngCore};

#[derive(Clone, Copy, Debug, Default)]
pub struct VeilidRng;

impl CryptoRng for VeilidRng {}

impl RngCore for VeilidRng {
    fn next_u32(&mut self) -> u32 {
        intf::get_random_u32()
    }

    fn next_u64(&mut self) -> u64 {
        intf::get_random_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        if let Err(e) = self.try_fill_bytes(dest) {
            panic!("Error: {}", e);
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        intf::random_bytes(dest).map_err(|err| Error::new(err))
    }
}
