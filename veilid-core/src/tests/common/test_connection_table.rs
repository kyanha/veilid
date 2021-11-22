use crate::connection_table::*;
use crate::intf::*;
use crate::xx::*;
use crate::*;

pub async fn test_add_get_remove() {
    let table = ConnectionTable::new();

    let c1 = NetworkConnection::Dummy(DummyNetworkConnection {});
    let c2 = NetworkConnection::Dummy(DummyNetworkConnection {});
    let c3 = NetworkConnection::Dummy(DummyNetworkConnection {});

    let a1 = ConnectionDescriptor::new_no_local(PeerAddress::new(
        Address::IPV4(Ipv4Addr::new(127, 0, 0, 1)),
        8080,
        ProtocolType::TCP,
    ));
    let a2 = ConnectionDescriptor::new_no_local(PeerAddress::new(
        Address::IPV4(Ipv4Addr::new(127, 0, 0, 1)),
        8080,
        ProtocolType::TCP,
    ));
    let a3 = ConnectionDescriptor::new(
        PeerAddress::new(
            Address::IPV6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)),
            8090,
            ProtocolType::TCP,
        ),
        SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1),
            8080,
            0,
            0,
        )),
    );
    let a4 = ConnectionDescriptor::new(
        PeerAddress::new(
            Address::IPV6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)),
            8090,
            ProtocolType::TCP,
        ),
        SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::new(1, 0, 0, 0, 0, 0, 0, 1),
            8080,
            0,
            0,
        )),
    );
    let a5 = ConnectionDescriptor::new(
        PeerAddress::new(
            Address::Hostname("example.com".to_owned()),
            8090,
            ProtocolType::WSS,
        ),
        SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1),
            8080,
            0,
            0,
        )),
    );

    assert_eq!(a1, a2);
    assert_ne!(a3, a4);
    assert_ne!(a4, a5);

    assert_eq!(table.connection_count(), 0);
    assert_eq!(table.get_connection(&a1), None);
    let entry1 = table.add_connection(a1.clone(), c1.clone()).unwrap();

    assert_eq!(table.connection_count(), 1);
    assert_eq!(table.remove_connection(&a3), Err(()));
    assert_eq!(table.remove_connection(&a4), Err(()));
    assert_eq!(table.connection_count(), 1);
    assert_eq!(table.get_connection(&a1), Some(entry1.clone()));
    assert_eq!(table.get_connection(&a1), Some(entry1.clone()));
    assert_eq!(table.connection_count(), 1);
    assert_eq!(table.add_connection(a1.clone(), c1.clone()), Err(()));
    assert_eq!(table.add_connection(a1.clone(), c2.clone()), Err(()));
    assert_eq!(table.connection_count(), 1);
    assert_eq!(table.get_connection(&a1), Some(entry1.clone()));
    assert_eq!(table.get_connection(&a1), Some(entry1.clone()));
    assert_eq!(table.connection_count(), 1);
    assert_eq!(table.remove_connection(&a2), Ok(entry1.clone()));
    assert_eq!(table.connection_count(), 0);
    assert_eq!(table.remove_connection(&a2), Err(()));
    assert_eq!(table.connection_count(), 0);
    assert_eq!(table.get_connection(&a2), None);
    assert_eq!(table.get_connection(&a1), None);
    assert_eq!(table.connection_count(), 0);
    let entry2 = table.add_connection(a1.clone(), c1.clone()).unwrap();
    assert_eq!(table.add_connection(a2.clone(), c1), Err(()));
    let entry3 = table.add_connection(a3.clone(), c2.clone()).unwrap();
    let entry4 = table.add_connection(a4.clone(), c3.clone()).unwrap();
    assert_eq!(table.connection_count(), 3);
    assert_eq!(table.remove_connection(&a2), Ok(entry2.clone()));
    assert_eq!(table.remove_connection(&a3), Ok(entry3.clone()));
    assert_eq!(table.remove_connection(&a4), Ok(entry4.clone()));
    assert_eq!(table.connection_count(), 0);
}

pub async fn test_all() {
    test_add_get_remove().await;
}
