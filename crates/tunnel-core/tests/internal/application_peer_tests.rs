//! Existing browser HTTP/target seam exercised over ordinary upstream peer connections.
use super::*;
use crate::http::{host::Counts, inspector::HttpInspector};
use saorsa_transport::crypto::{
    pqc::types::MlDsaPublicKey, raw_public_keys::fingerprint_public_key,
};
use saorsa_transport::{NatConfig, P2pConfig, P2pEndpoint, upnp::UpnpConfig};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    task::JoinSet,
};

async fn node() -> P2pEndpoint {
    P2pEndpoint::new(
        P2pConfig::builder()
            .bind_addr("127.0.0.1:0".parse::<SocketAddr>().unwrap())
            .max_connections(4)
            .nat(NatConfig {
                allow_loopback: true,
                enable_relay_service: false,
                advertise_external_addresses: false,
                upnp: UpnpConfig::disabled(),
                ..NatConfig::default()
            })
            .build()
            .unwrap(),
    )
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browser_http_and_stop_use_the_existing_peer_connection() {
    let computer = node().await;
    let phone = node().await;
    let mut tasks = JoinSet::new();
    let outcome=timeout(Duration::from_secs(20),async {
        let target=TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port=target.local_addr().unwrap().port();
        let (inspector,grant)=HttpInspector::new(port).unwrap();
        let grant=Zeroizing::new(grant);
        let address=computer.local_addr().unwrap();
        phone.connect(address).await.unwrap();
        let peer=computer.accept().await.unwrap();
        let accepted=computer.get_quic_connection(&peer.remote_addr.as_socket_addr().unwrap()).await.unwrap().unwrap();
        let connection=phone.get_quic_connection(&address).await.unwrap().unwrap();
        let expected=fingerprint_public_key(&MlDsaPublicKey::from_bytes(computer.public_key_bytes()).unwrap());
        let (counts_tx,counts_rx)=tokio::sync::oneshot::channel();
        tasks.spawn(async move {
            let mut counts=Counts::default();
            let result=serve_peer(accepted,inspector,port,&mut counts).await;
            let _=counts_tx.send((result,counts));
        });
        tasks.spawn(async move {
            let(mut socket,_)=target.accept().await.unwrap();
            let mut request=Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                request.push(socket.read_u8().await.unwrap());assert!(request.len()<16384);
            }
            assert!(request.starts_with(b"GET /fixture HTTP/1.1\r\n"));
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\npeer fixture").await.unwrap();
        });
        let session=Session::from_peer(connection,expected,&grant).await?;
        drop(grant);
        let listener=TcpListener::bind("127.0.0.1:0").await.unwrap();
        let browser=listener.local_addr().unwrap();
        let hostname=format!("thepeer-{}.localhost",hex::encode(rand::random::<[u8;16]>()));
        let authority=format!("{hostname}:{}",browser.port());
        let(stop,stopped)=tokio::sync::oneshot::channel();
        tasks.spawn(async move { drive(listener,session,hostname,async {let _=stopped.await;}).await.unwrap(); });
        let response=reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(5)).build().unwrap()
            .get(format!("http://{browser}/fixture")).header("Host",authority).send().await.unwrap();
        assert_eq!(response.status(),200);
        assert!(response.headers()["content-security-policy"].to_str().unwrap().contains("script-src 'self'"));
        assert_eq!(response.text().await.unwrap(),"peer fixture");
        let _=stop.send(());
        let(result,counts)=counts_rx.await.unwrap();result?;
        assert!(counts.authorized);assert_eq!(counts.fetched,1);
        while let Some(done)=tasks.join_next().await{done.unwrap();}
        assert!(tokio::net::TcpStream::connect(browser).await.is_err());
        Ok::<(),Failure>(())
    }).await;
    tasks.shutdown().await;
    phone.shutdown().await;
    computer.shutdown().await;
    outcome
        .expect("bounded HTTP/Stop fixture")
        .expect("peer application forwarding");
}
