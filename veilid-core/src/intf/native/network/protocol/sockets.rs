use crate::xx::*;
use crate::*;
use socket2::{Domain, Protocol, Socket, Type};

cfg_if! {
    if #[cfg(windows)] {
        use winapi::shared::ws2def::{ SOL_SOCKET, SO_EXCLUSIVEADDRUSE};
        use winapi::um::winsock2::{SOCKET_ERROR, setsockopt};
        use winapi::ctypes::c_int;
        use std::os::windows::io::AsRawSocket;

        fn set_exclusiveaddruse(socket: &Socket) -> Result<(), String> {
            unsafe {
                let optval:c_int = 1;
                if setsockopt(socket.as_raw_socket().try_into().unwrap(), SOL_SOCKET, SO_EXCLUSIVEADDRUSE, (&optval as *const c_int).cast(),
                    std::mem::size_of::<c_int>() as c_int) == SOCKET_ERROR {
                    return Err("Unable to SO_EXCLUSIVEADDRUSE".to_owned());
                }
                Ok(())
            }
        }
    }
}

pub fn new_unbound_shared_udp_socket(domain: Domain) -> Result<Socket, String> {
    let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))
        .map_err(|e| format!("Couldn't create UDP socket: {}", e))?;
    if domain == Domain::IPV6 {
        socket
            .set_only_v6(true)
            .map_err(|e| format!("Couldn't set IPV6_V6ONLY: {}", e))?;
    }
    socket
        .set_reuse_address(true)
        .map_err(|e| format!("Couldn't set reuse address: {}", e))?;
    cfg_if! {
        if #[cfg(unix)] {
            socket.set_reuse_port(true).map_err(|e| format!("Couldn't set reuse port: {}", e))?;
        }
    }
    Ok(socket)
}

pub fn new_bound_shared_udp_socket(local_address: SocketAddr) -> Result<Socket, String> {
    let domain = Domain::for_address(local_address);
    let socket = new_unbound_shared_udp_socket(domain)?;
    let socket2_addr = socket2::SockAddr::from(local_address);
    socket.bind(&socket2_addr).map_err(|e| {
        format!(
            "failed to bind UDP socket to '{}' in domain '{:?}': {} ",
            local_address, domain, e
        )
    })?;

    log_net!("created bound shared udp socket on {:?}", &local_address);

    Ok(socket)
}

pub fn new_bound_first_udp_socket(local_address: SocketAddr) -> Result<Socket, String> {
    let domain = Domain::for_address(local_address);
    let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))
        .map_err(|e| format!("Couldn't create UDP socket: {}", e))?;
    if domain == Domain::IPV6 {
        socket
            .set_only_v6(true)
            .map_err(|e| format!("Couldn't set IPV6_V6ONLY: {}", e))?;
    }
    // Bind the socket -first- before turning on 'reuse address' this way it will
    // fail if the port is already taken
    let socket2_addr = socket2::SockAddr::from(local_address);

    // On windows, do SO_EXCLUSIVEADDRUSE before the bind to ensure the port is fully available
    cfg_if! {
        if #[cfg(windows)] {
            set_exclusiveaddruse(&socket)?;
        }
    }

    socket
        .bind(&socket2_addr)
        .map_err(|e| format!("failed to bind UDP socket: {}", e))?;

    // Set 'reuse address' so future binds to this port will succeed
    // This does not work on Windows, where reuse options can not be set after the bind
    cfg_if! {
        if #[cfg(unix)] {
            socket
                .set_reuse_address(true)
                .map_err(|e| format!("Couldn't set reuse address: {}", e))?;
            socket.set_reuse_port(true).map_err(|e| format!("Couldn't set reuse port: {}", e))?;
        }
    }
    log_net!("created bound first udp socket on {:?}", &local_address);

    Ok(socket)
}

pub fn new_unbound_shared_tcp_socket(domain: Domain) -> Result<Socket, String> {
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))
        .map_err(map_to_string)
        .map_err(logthru_net!("failed to create TCP socket"))?;
    if let Err(e) = socket.set_linger(Some(core::time::Duration::from_secs(0))) {
        log_net!(error "Couldn't set TCP linger: {}", e);
    }
    if let Err(e) = socket.set_nodelay(true) {
        log_net!(error "Couldn't set TCP nodelay: {}", e);
    }
    if domain == Domain::IPV6 {
        socket
            .set_only_v6(true)
            .map_err(|e| format!("Couldn't set IPV6_V6ONLY: {}", e))?;
    }
    socket
        .set_reuse_address(true)
        .map_err(|e| format!("Couldn't set reuse address: {}", e))?;
    cfg_if! {
        if #[cfg(unix)] {
            socket.set_reuse_port(true).map_err(|e| format!("Couldn't set reuse port: {}", e))?;
        }
    }

    Ok(socket)
}

pub fn new_bound_shared_tcp_socket(local_address: SocketAddr) -> Result<Socket, String> {
    let domain = Domain::for_address(local_address);

    let socket = new_unbound_shared_tcp_socket(domain)?;

    let socket2_addr = socket2::SockAddr::from(local_address);
    socket
        .bind(&socket2_addr)
        .map_err(|e| format!("failed to bind TCP socket: {}", e))?;

    log_net!("created bound shared tcp socket on {:?}", &local_address);

    Ok(socket)
}

pub fn new_bound_first_tcp_socket(local_address: SocketAddr) -> Result<Socket, String> {
    let domain = Domain::for_address(local_address);

    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))
        .map_err(map_to_string)
        .map_err(logthru_net!("failed to create TCP socket"))?;
    if let Err(e) = socket.set_linger(Some(core::time::Duration::from_secs(0))) {
        log_net!(error "Couldn't set TCP linger: {}", e);
    }
    if let Err(e) = socket.set_nodelay(true) {
        log_net!(error "Couldn't set TCP nodelay: {}", e);
    }
    if domain == Domain::IPV6 {
        socket
            .set_only_v6(true)
            .map_err(|e| format!("Couldn't set IPV6_V6ONLY: {}", e))?;
    }

    // On windows, do SO_EXCLUSIVEADDRUSE before the bind to ensure the port is fully available
    cfg_if! {
        if #[cfg(windows)] {
            set_exclusiveaddruse(&socket)?;
        }
    }

    // Bind the socket -first- before turning on 'reuse address' this way it will
    // fail if the port is already taken
    let socket2_addr = socket2::SockAddr::from(local_address);
    socket
        .bind(&socket2_addr)
        .map_err(|e| format!("failed to bind TCP socket: {}", e))?;

    // Set 'reuse address' so future binds to this port will succeed
    // This does not work on Windows, where reuse options can not be set after the bind
    cfg_if! {
        if #[cfg(unix)] {
        socket
            .set_reuse_address(true)
            .map_err(|e| format!("Couldn't set reuse address: {}", e))?;
        socket.set_reuse_port(true).map_err(|e| format!("Couldn't set reuse port: {}", e))?;
        }
    }
    log_net!("created bound first tcp socket on {:?}", &local_address);

    Ok(socket)
}
