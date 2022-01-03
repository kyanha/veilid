use crate::connection_manager::*;
use crate::connection_table::*;
use crate::intf::*;
use crate::xx::*;
use crate::*;

pub async fn test_add_get_remove() {
    let mut table = ConnectionTable::new();

    let a1 = ConnectionDescriptor::new_no_local(PeerAddress::new(
        SocketAddress::new(Address::IPV4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
        ProtocolType::TCP,
    ));
    let a2 = a1.clone();
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

    let c1 = NetworkConnection::Dummy(DummyNetworkConnection::new(a1.clone()));
    let c2 = NetworkConnection::Dummy(DummyNetworkConnection::new(a2.clone()));
    let c3 = NetworkConnection::Dummy(DummyNetworkConnection::new(a3.clone()));
    let c4 = NetworkConnection::Dummy(DummyNetworkConnection::new(a4.clone()));
    let c5 = NetworkConnection::Dummy(DummyNetworkConnection::new(a5));

    assert_eq!(a1, c2.connection_descriptor());
    assert_ne!(a3, c4.connection_descriptor());
    assert_ne!(a4, c5.connection_descriptor());

    assert_eq!(table.connection_count(), 0);
    assert_eq!(table.get_connection(&a1), None);
    let entry1 = table.add_connection(c1.clone()).unwrap();

    assert_eq!(table.connection_count(), 1);
    assert_err!(table.remove_connection(&a3));
    assert_err!(table.remove_connection(&a4));
    assert_eq!(table.connection_count(), 1);
    assert_eq!(table.get_connection(&a1), Some(entry1.clone()));
    assert_eq!(table.get_connection(&a1), Some(entry1.clone()));
    assert_eq!(table.connection_count(), 1);
    assert_err!(table.add_connection(c1.clone()));
    assert_err!(table.add_connection(c2.clone()));
    assert_eq!(table.connection_count(), 1);
    assert_eq!(table.get_connection(&a1), Some(entry1.clone()));
    assert_eq!(table.get_connection(&a1), Some(entry1.clone()));
    assert_eq!(table.connection_count(), 1);
    assert_eq!(table.remove_connection(&a2), Ok(entry1));
    assert_eq!(table.connection_count(), 0);
    assert_err!(table.remove_connection(&a2));
    assert_eq!(table.connection_count(), 0);
    assert_eq!(table.get_connection(&a2), None);
    assert_eq!(table.get_connection(&a1), None);
    assert_eq!(table.connection_count(), 0);
    let entry2 = table.add_connection(c1).unwrap();
    assert_err!(table.add_connection(c2));
    let entry3 = table.add_connection(c3).unwrap();
    let entry4 = table.add_connection(c4).unwrap();
    assert_eq!(table.connection_count(), 3);
    assert_eq!(table.remove_connection(&a2), Ok(entry2));
    assert_eq!(table.remove_connection(&a3), Ok(entry3));
    assert_eq!(table.remove_connection(&a4), Ok(entry4));
    assert_eq!(table.connection_count(), 0);
}

pub async fn test_all() {
    test_add_get_remove().await;
}
