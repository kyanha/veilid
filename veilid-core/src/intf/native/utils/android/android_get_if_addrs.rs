use super::*;
use crate::xx::*;
pub use if_addrs::{IfAddr, Ifv4Addr, Ifv6Addr, Interface};
use jni::objects::JValue;
use std::io;

fn get_netmask_from_prefix_length_v4(out: &mut [u8; 4], mut plen: i16) {
    for n in 0..4 {
        out[n] = if plen >= 8 {
            plen -= 8;
            255u8
        } else if plen <= 0 {
            0u8
        } else {
            let v = 255u8 << (8 - plen);
            plen = 0;
            v
        }
    }
}
fn get_netmask_from_prefix_length_v6(out: &mut [u8; 16], mut plen: i16) {
    for n in 0..16 {
        out[n] = if plen >= 8 {
            plen -= 8;
            255u8
        } else if plen == 0 {
            0u8
        } else {
            let v = 255u8 << (8 - plen);
            plen = 0;
            v
        }
    }
}

fn convert_to_unsigned_4(x: [i8; 4]) -> [u8; 4] {
    let mut out: [u8; 4] = [0u8; 4];
    for i in 0..4 {
        out[i] = x[i] as u8;
    }
    out
}

fn convert_to_unsigned_16(x: [i8; 16]) -> [u8; 16] {
    let mut out: [u8; 16] = [0u8; 16];
    for i in 0..16 {
        out[i] = x[i] as u8;
    }
    out
}

macro_rules! call_method_checked {
    ($env:expr, $obj:expr, $name:expr, $sig:expr, $args:expr, $kind:ident) => {
        $env.call_method($obj, $name, $sig, $args)
            .map_err(|e| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!("call_method {} {} failed: {}", $name, $sig, e),
                )
            })?
            .$kind()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
    };
}

pub fn get_if_addrs() -> io::Result<Vec<Interface>> {
    let aglock = ANDROID_GLOBALS.lock();
    let ag = aglock.as_ref().unwrap();
    let env = ag.vm.attach_current_thread().unwrap();

    let niclass = env.find_class("java/net/NetworkInterface").unwrap();
    let intfenum = env
        .call_static_method(
            niclass,
            "getNetworkInterfaces",
            "()Ljava/util/Enumeration;",
            &[],
        )
        .unwrap()
        .l()
        .unwrap();

    let mut out: Vec<Interface> = Vec::new();
    while call_method_checked!(env, intfenum, "hasMoreElements", "()Z", &[], z) {
        let intf =
            call_method_checked!(env, intfenum, "nextElement", "()Ljava/lang/Object;", &[], l);

        let nameobj = call_method_checked!(env, intf, "getName", "()Ljava/lang/String;", &[], l);
        let name_jstrval = env.get_string(JString::from(nameobj)).unwrap();
        let name = String::from(name_jstrval.to_string_lossy());

        let intfaddrs = call_method_checked!(
            env,
            intf,
            "getInterfaceAddresses",
            "()Ljava/util/List;",
            &[],
            l
        );
        let size = call_method_checked!(env, intfaddrs, "size", "()I", &[], i);
        for i in 0..size {
            let intfaddr = call_method_checked!(
                env,
                intfaddrs,
                "get",
                "(I)Ljava/lang/Object;",
                &[JValue::Int(i)],
                l
            );

            let ia_addr = call_method_checked!(
                env,
                intfaddr,
                "getAddress",
                "()Ljava/net/InetAddress;",
                &[],
                l
            );
            let ia_bcst = call_method_checked!(
                env,
                intfaddr,
                "getBroadcast",
                "()Ljava/net/InetAddress;",
                &[],
                l
            );
            let ia_plen =
                call_method_checked!(env, intfaddr, "getNetworkPrefixLength", "()S", &[], s);

            let ia_addr_bytearray =
                call_method_checked!(env, ia_addr, "getAddress", "()[B", &[], l);
            let ia_addr_bytearray_len = env.get_array_length(*ia_addr_bytearray).unwrap();
            let addr: IfAddr;
            if ia_addr_bytearray_len == 4 {
                let mut ia_addr_bytes_v4 = [0i8; 4];
                env.get_byte_array_region(*ia_addr_bytearray, 0, &mut ia_addr_bytes_v4)
                    .unwrap();

                let broadcast = if !env.is_same_object(ia_bcst, JObject::null()).unwrap() {
                    let ia_bcst_bytearray =
                        call_method_checked!(env, ia_bcst, "getAddress", "()[B", &[], l);
                    let ia_bcst_bytearray_len = env.get_array_length(*ia_bcst_bytearray).unwrap();
                    if ia_bcst_bytearray_len != 4 {
                        warn!(
                            "mismatched inet4 broadcast address length: {}",
                            ia_bcst_bytearray_len
                        );
                        continue;
                    }

                    let mut ia_bsct_bytes_v4 = [0i8; 4];
                    env.get_byte_array_region(*ia_bcst_bytearray, 0, &mut ia_bsct_bytes_v4)
                        .unwrap();

                    Some(Ipv4Addr::from(convert_to_unsigned_4(ia_bsct_bytes_v4)))
                } else {
                    None
                };

                let mut ia_netmask_bytes_v4 = [0u8; 4];
                get_netmask_from_prefix_length_v4(&mut ia_netmask_bytes_v4, ia_plen);
                addr = IfAddr::V4(Ifv4Addr {
                    ip: Ipv4Addr::from(convert_to_unsigned_4(ia_addr_bytes_v4)),
                    netmask: Ipv4Addr::from(ia_netmask_bytes_v4),
                    broadcast: broadcast,
                });
            } else if ia_addr_bytearray_len == 16 {
                let mut ia_addr_bytes_v6 = [0i8; 16];
                env.get_byte_array_region(*ia_addr_bytearray, 0, &mut ia_addr_bytes_v6)
                    .unwrap();

                let mut ia_netmask_bytes_v6 = [0u8; 16];
                get_netmask_from_prefix_length_v6(&mut ia_netmask_bytes_v6, ia_plen);
                addr = IfAddr::V6(Ifv6Addr {
                    ip: Ipv6Addr::from(convert_to_unsigned_16(ia_addr_bytes_v6)),
                    netmask: Ipv6Addr::from(ia_netmask_bytes_v6),
                    broadcast: None,
                });
            } else {
                warn!("weird inet address length: {}", ia_addr_bytearray_len);
                continue;
            }
            let elem = Interface {
                name: name.clone(),
                addr: addr,
            };

            out.push(elem);
        }
    }
    Ok(out)
}
