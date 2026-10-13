use super::*;
use std::{net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn configured_literal_uses_only_that_loopback_family() {
    for bind in ["127.0.0.1:0", "[::1]:0"] {
        let listener = TcpListener::bind(bind).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let target: Target = addr.to_string().parse().unwrap();
        let prepared = target.prepare(&CancellationToken::new()).await.unwrap();
        let mut client = prepared.connect().await.unwrap();
        let (mut server, _) = listener.accept().await.unwrap();
        client.write_all(b"local").await.unwrap();
        let mut bytes = [0; 5];
        server.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"local");
        drop(listener);
        assert!(prepared.connect().await.is_err());
    }
}

#[tokio::test]
async fn hostname_is_resolved_once_and_connections_use_pinned_addresses() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let target: Target = format!("custom.test:{}", addr.port()).parse().unwrap();
    // Inject only the OS resolution result, not the connector or HTTP behavior.
    // custom.test is deliberately not configured in DNS or this machine's hosts file.
    let prepared = target
        .prepare_with(&CancellationToken::new(), async { Ok(vec![addr]) })
        .await
        .unwrap();
    for _ in 0..2 {
        let client = prepared.connect().await.unwrap();
        let (server, _) = listener.accept().await.unwrap();
        assert_eq!(client.peer_addr().unwrap(), addr);
        drop((server, client));
    }
    assert_eq!(
        prepared.target.authority(),
        format!("custom.test:{}", addr.port())
    );
}

#[tokio::test]
async fn mixed_empty_nonlocal_and_excessive_answers_are_rejected_before_connecting() {
    let stop = CancellationToken::new();
    let local: SocketAddr = "127.0.0.1:4321".parse().unwrap();
    for answers in [
        vec![],
        vec![local, "192.0.2.1:4321".parse().unwrap()],
        vec![local, "192.168.1.1:4321".parse().unwrap()],
        vec!["0.0.0.0:4321".parse().unwrap()],
        vec!["[::]:4321".parse().unwrap()],
        vec!["[::ffff:127.0.0.1]:4321".parse().unwrap()],
        vec!["127.0.0.1:4322".parse().unwrap()],
        vec![local; 17],
    ] {
        let target: Target = "custom.test:4321".parse().unwrap();
        assert!(matches!(
            target.prepare_with(&stop, async { Ok(answers) }).await,
            Err(Failure("target-not-loopback"))
        ));
    }
}

#[tokio::test(start_paused = true)]
async fn resolution_deadline_and_stop_are_terminal() {
    let target: Target = "custom.test:4321".parse().unwrap();
    let stop = CancellationToken::new();
    let start = tokio::time::Instant::now();
    assert!(matches!(
        target
            .clone()
            .prepare_with(&stop, std::future::pending())
            .await,
        Err(Failure("target-resolution-timeout"))
    ));
    assert_eq!(start.elapsed(), Duration::from_secs(5));
    stop.cancel();
    assert!(matches!(
        target
            .prepare_with(&stop, async { panic!("cancelled lookup was polled") })
            .await,
        Err(Failure("peer-cancelled"))
    ));
}
