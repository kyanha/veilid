use crate::xx::*;
use async_std::io;
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task;
use std::time::Duration;

static MESSAGE: &[u8; 62] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

async fn make_tcp_loopback() -> Result<(TcpStream, TcpStream), io::Error> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_addr = listener.local_addr()?;

    let accept_future = async {
        let (accepted_stream, peer_address) = listener.accept().await?;
        trace!("connection from {}", peer_address);
        accepted_stream.set_nodelay(true)?;
        Result::<TcpStream, io::Error>::Ok(accepted_stream)
    };
    let connect_future = async {
        task::sleep(Duration::from_secs(1)).await;
        let connected_stream = TcpStream::connect(local_addr).await?;
        connected_stream.set_nodelay(true)?;
        Result::<TcpStream, io::Error>::Ok(connected_stream)
    };

    Ok(accept_future.try_join(connect_future).await?)
}

async fn make_async_peek_stream_loopback() -> (AsyncPeekStream, AsyncPeekStream) {
    let (acc, conn) = make_tcp_loopback().await.unwrap();
    let aps_a = AsyncPeekStream::new(acc);
    let aps_c = AsyncPeekStream::new(conn);

    (aps_a, aps_c)
}

async fn make_tcpstream_loopback() -> (TcpStream, TcpStream) {
    make_tcp_loopback().await.unwrap()
}

pub async fn test_nothing() {
    info!("test_nothing");
    let (mut a, mut c) = make_tcpstream_loopback().await;
    let outbuf = MESSAGE.to_vec();

    a.write_all(&outbuf).await.unwrap();

    let mut inbuf: Vec<u8> = Vec::new();
    inbuf.resize(outbuf.len(), 0u8);
    c.read_exact(&mut inbuf).await.unwrap();

    assert_eq!(inbuf, outbuf);
}

pub async fn test_no_peek() {
    info!("test_no_peek");
    let (mut a, mut c) = make_async_peek_stream_loopback().await;

    let outbuf = MESSAGE.to_vec();

    a.write_all(&outbuf).await.unwrap();

    let mut inbuf: Vec<u8> = Vec::new();
    inbuf.resize(outbuf.len(), 0u8);
    c.read_exact(&mut inbuf).await.unwrap();

    assert_eq!(inbuf, outbuf);
}

pub async fn test_peek_all_read() {
    info!("test_peek_all_read");

    let (mut a, mut c) = make_async_peek_stream_loopback().await;
    // write everything
    let outbuf = MESSAGE.to_vec();
    a.write_all(&outbuf).await.unwrap();

    // peek everything
    let mut peekbuf1: Vec<u8> = Vec::new();
    peekbuf1.resize(outbuf.len(), 0u8);
    let peeksize1 = c.peek(&mut peekbuf1).await.unwrap();

    assert_eq!(peeksize1, peekbuf1.len());
    // read everything
    let mut inbuf: Vec<u8> = Vec::new();
    inbuf.resize(outbuf.len(), 0u8);
    c.read_exact(&mut inbuf).await.unwrap();

    assert_eq!(inbuf, outbuf);
    assert_eq!(peekbuf1, outbuf);
}

pub async fn test_peek_some_read() {
    info!("test_peek_some_read");

    let (mut a, mut c) = make_async_peek_stream_loopback().await;

    // write everything
    let outbuf = MESSAGE.to_vec();
    a.write_all(&outbuf).await.unwrap();

    // peek partially
    let mut peekbuf1: Vec<u8> = Vec::new();
    peekbuf1.resize(outbuf.len() / 2, 0u8);
    let peeksize1 = c.peek(&mut peekbuf1).await.unwrap();
    assert_eq!(peeksize1, peekbuf1.len());
    // read everything
    let mut inbuf: Vec<u8> = Vec::new();
    inbuf.resize(outbuf.len(), 0u8);
    c.read_exact(&mut inbuf).await.unwrap();

    assert_eq!(inbuf, outbuf);
    assert_eq!(peekbuf1, outbuf[0..peeksize1].to_vec());
}

pub async fn test_peek_some_peek_some_read() {
    info!("test_peek_some_peek_some_read");

    let (mut a, mut c) = make_async_peek_stream_loopback().await;

    // write everything
    let outbuf = MESSAGE.to_vec();
    a.write_all(&outbuf).await.unwrap();

    // peek partially
    let mut peekbuf1: Vec<u8> = Vec::new();
    peekbuf1.resize(outbuf.len() / 4, 0u8);
    let peeksize1 = c.peek(&mut peekbuf1).await.unwrap();
    assert_eq!(peeksize1, peekbuf1.len());

    // peek partially
    let mut peekbuf2: Vec<u8> = Vec::new();
    peekbuf2.resize(peeksize1 + 1, 0u8);
    let peeksize2 = c.peek(&mut peekbuf2).await.unwrap();
    assert_eq!(peeksize2, peekbuf2.len());

    // read everything
    let mut inbuf: Vec<u8> = Vec::new();
    inbuf.resize(outbuf.len(), 0u8);
    c.read_exact(&mut inbuf).await.unwrap();

    assert_eq!(inbuf, outbuf);
    assert_eq!(peekbuf1, outbuf[0..peeksize1].to_vec());
    assert_eq!(peekbuf2, outbuf[0..peeksize2].to_vec());
}

pub async fn test_peek_some_read_peek_some_read() {
    info!("test_peek_some_read_peek_some_read");

    let (mut a, mut c) = make_async_peek_stream_loopback().await;

    // write everything
    let outbuf = MESSAGE.to_vec();
    a.write_all(&outbuf).await.unwrap();

    // peek partially
    let mut peekbuf1: Vec<u8> = Vec::new();
    peekbuf1.resize(outbuf.len() / 4, 0u8);
    let peeksize1 = c.peek(&mut peekbuf1).await.unwrap();
    assert_eq!(peeksize1, peekbuf1.len());

    // read partially
    let mut inbuf1: Vec<u8> = Vec::new();
    inbuf1.resize(peeksize1 - 1, 0u8);
    c.read_exact(&mut inbuf1).await.unwrap();

    // peek partially
    let mut peekbuf2: Vec<u8> = Vec::new();
    peekbuf2.resize(2, 0u8);
    let peeksize2 = c.peek(&mut peekbuf2).await.unwrap();
    assert_eq!(peeksize2, peekbuf2.len());

    // read partially
    let mut inbuf2: Vec<u8> = Vec::new();
    inbuf2.resize(2, 0u8);
    c.read_exact(&mut inbuf2).await.unwrap();

    assert_eq!(peekbuf1, outbuf[0..peeksize1].to_vec());
    assert_eq!(inbuf1, outbuf[0..peeksize1 - 1].to_vec());
    assert_eq!(peekbuf2, outbuf[peeksize1 - 1..peeksize1 + 1].to_vec());
    assert_eq!(inbuf2, peekbuf2);
}

pub async fn test_peek_some_read_peek_all_read() {
    info!("test_peek_some_read_peek_all_read");

    let (mut a, mut c) = make_async_peek_stream_loopback().await;

    // write everything
    let outbuf = MESSAGE.to_vec();
    a.write_all(&outbuf).await.unwrap();

    // peek partially
    let mut peekbuf1: Vec<u8> = Vec::new();
    peekbuf1.resize(outbuf.len() / 4, 0u8);
    let peeksize1 = c.peek(&mut peekbuf1).await.unwrap();
    assert_eq!(peeksize1, peekbuf1.len());

    // read partially
    let mut inbuf1: Vec<u8> = Vec::new();
    inbuf1.resize(peeksize1 + 1, 0u8);
    c.read_exact(&mut inbuf1).await.unwrap();

    // peek past end
    let mut peekbuf2: Vec<u8> = Vec::new();
    peekbuf2.resize(outbuf.len(), 0u8);
    let peeksize2 = c.peek(&mut peekbuf2).await.unwrap();
    assert_eq!(peeksize2, outbuf.len() - (peeksize1 + 1));

    // read remaining
    let mut inbuf2: Vec<u8> = Vec::new();
    inbuf2.resize(peeksize2, 0u8);
    c.read_exact(&mut inbuf2).await.unwrap();

    assert_eq!(peekbuf1, outbuf[0..peeksize1].to_vec());
    assert_eq!(inbuf1, outbuf[0..peeksize1 + 1].to_vec());
    assert_eq!(
        peekbuf2[0..peeksize2].to_vec(),
        outbuf[peeksize1 + 1..outbuf.len()].to_vec()
    );
    assert_eq!(inbuf2, peekbuf2[0..peeksize2].to_vec());
}

pub async fn test_peek_some_read_peek_some_read_all_read() {
    info!("test_peek_some_read_peek_some_read_peek_all_read");

    let (mut a, mut c) = make_async_peek_stream_loopback().await;

    // write everything
    let outbuf = MESSAGE.to_vec();
    a.write_all(&outbuf).await.unwrap();

    // peek partially
    let mut peekbuf1: Vec<u8> = Vec::new();
    peekbuf1.resize(outbuf.len() / 4, 0u8);
    let peeksize1 = c.peek(&mut peekbuf1).await.unwrap();
    assert_eq!(peeksize1, peekbuf1.len());

    // read partially
    let mut inbuf1: Vec<u8> = Vec::new();
    inbuf1.resize(peeksize1 - 1, 0u8);
    c.read_exact(&mut inbuf1).await.unwrap();

    // peek partially
    let mut peekbuf2: Vec<u8> = Vec::new();
    peekbuf2.resize(2, 0u8);
    let peeksize2 = c.peek(&mut peekbuf2).await.unwrap();
    assert_eq!(peeksize2, peekbuf2.len());
    // read partially
    let mut inbuf2: Vec<u8> = Vec::new();
    inbuf2.resize(1, 0u8);
    c.read_exact(&mut inbuf2).await.unwrap();

    // read remaining
    let mut inbuf3: Vec<u8> = Vec::new();
    inbuf3.resize(outbuf.len() - peeksize1, 0u8);
    c.read_exact(&mut inbuf3).await.unwrap();

    assert_eq!(peekbuf1, outbuf[0..peeksize1].to_vec());
    assert_eq!(inbuf1, outbuf[0..peeksize1 - 1].to_vec());
    assert_eq!(
        peekbuf2[0..peeksize2].to_vec(),
        outbuf[peeksize1 - 1..peeksize1 + 1].to_vec()
    );
    assert_eq!(inbuf2, peekbuf2[0..1].to_vec());
    assert_eq!(inbuf3, outbuf[peeksize1..outbuf.len()].to_vec());
}

pub async fn test_all() {
    test_nothing().await;
    test_no_peek().await;
    test_peek_all_read().await;
    test_peek_some_read().await;
    test_peek_some_peek_some_read().await;
    test_peek_some_read_peek_some_read().await;
    test_peek_some_read_peek_all_read().await;
    test_peek_some_read_peek_some_read_all_read().await;
}
