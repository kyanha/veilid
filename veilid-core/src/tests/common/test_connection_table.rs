use super::test_veilid_config::*;
use crate::connection_table::*;
use crate::network_connection::*;
use crate::xx::*;
use crate::*;

pub async fn test_add_get_remove() {
    let config = get_config();

    let mut table = ConnectionTable::new(config);

    let a1 = ConnectionDescriptor::new_no_local(PeerAddress::new(
        SocketAddress::new(Address::IPV4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
        ProtocolType::TCP,
    ));
    let a2 = a1;
    let a3 = ConnectionDescriptor::new(
        PeerAddress::new(
            SocketAddress::new(Address::IPV6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)), 8090),
            ProtocolType::TCP,
        ),
        SocketAddress::from_socket_addr(SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1),
            8080,
            0,
            0,
        ))),
    );
    let a4 = ConnectionDescriptor::new(
        PeerAddress::new(
            SocketAddress::new(Address::IPV6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)), 8090),
            ProtocolType::TCP,
        ),
        SocketAddress::from_socket_addr(SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::new(1, 0, 0, 0, 0, 0, 0, 1),
            8080,
            0,
            0,
        ))),
    );
    let a5 = ConnectionDescriptor::new(
        PeerAddress::new(
            SocketAddress::new(Address::IPV6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)), 8090),
            ProtocolType::WSS,
        ),
        SocketAddress::from_socket_addr(SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1),
            8080,
            0,
            0,
        ))),
    );

    let c1 = NetworkConnection::dummy(a1);
    let c2 = NetworkConnection::dummy(a2);
    let c3 = NetworkConnection::dummy(a3);
    let c4 = NetworkConnection::dummy(a4);
    let c5 = NetworkConnection::dummy(a5);

    assert_eq!(a1, c2.connection_descriptor());
    assert_ne!(a3, c4.connection_descriptor());
    assert_ne!(a4, c5.connection_descriptor());

    assert_eq!(table.connection_count(), 0);
    assert_eq!(table.get_connection(a1), None);
    table.add_connection(c1.clone()).unwrap();

    assert_eq!(table.connection_count(), 1);
    assert_err!(table.remove_connection(a3));
    assert_err!(table.remove_connection(a4));
    assert_eq!(table.connection_count(), 1);
    assert_eq!(table.get_connection(a1), Some(c1.clone()));
    assert_eq!(table.get_connection(a1), Some(c1.clone()));
    assert_eq!(table.connection_count(), 1);
    assert_err!(table.add_connection(c1.clone()));
    assert_err!(table.add_connection(c2.clone()));
    assert_eq!(table.connection_count(), 1);
    assert_eq!(table.get_connection(a1), Some(c1.clone()));
    assert_eq!(table.get_connection(a1), Some(c1.clone()));
    assert_eq!(table.connection_count(), 1);
    assert_eq!(table.remove_connection(a2), Ok(c1.clone()));
    assert_eq!(table.connection_count(), 0);
    assert_err!(table.remove_connection(a2));
    assert_eq!(table.connection_count(), 0);
    assert_eq!(table.get_connection(a2), None);
    assert_eq!(table.get_connection(a1), None);
    assert_eq!(table.connection_count(), 0);
    table.add_connection(c1.clone()).unwrap();
    assert_err!(table.add_connection(c2));
    table.add_connection(c3.clone()).unwrap();
    table.add_connection(c4.clone()).unwrap();
    assert_eq!(table.connection_count(), 3);
    assert_eq!(table.remove_connection(a2), Ok(c1));
    assert_eq!(table.remove_connection(a3), Ok(c3));
    assert_eq!(table.remove_connection(a4), Ok(c4));
    assert_eq!(table.connection_count(), 0);
}

pub async fn test_all() {
    test_add_get_remove().await;
}
