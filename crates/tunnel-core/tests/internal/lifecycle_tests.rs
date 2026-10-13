use super::*;
use tokio::net::UdpSocket;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn helper_shutdown_releases_listener_without_cancelling_parent() {
    timeout(Duration::from_secs(15), async {
        let parent = CancellationToken::new();
        let reservation = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let bind = reservation.local_addr().unwrap();
        drop(reservation);
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (helper, _offer) = Helper::prepare(
            HelperConfig {
                bind,
                target: format!("localhost:{}", target.local_addr().unwrap().port())
                    .parse()
                    .unwrap(),
                observer: None,
            },
            &parent,
        )
        .await
        .unwrap();
        assert!(UdpSocket::bind(bind).await.is_err());
        helper.shutdown().await.unwrap();
        assert!(!parent.is_cancelled());
        let _released = UdpSocket::bind(bind).await.unwrap();
        assert!(
            timeout(Duration::from_millis(100), target.accept())
                .await
                .is_err()
        );
    })
    .await
    .expect("bounded helper cleanup fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dropping_unpolled_browser_future_releases_phone_endpoint() {
    timeout(Duration::from_secs(15), async {
        let stop = CancellationToken::new();
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (helper, offer) = Helper::prepare(
            HelperConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                target: format!("localhost:{}", target.local_addr().unwrap().port())
                    .parse()
                    .unwrap(),
                observer: None,
            },
            &stop,
        )
        .await
        .unwrap();
        let reservation = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let bind = reservation.local_addr().unwrap();
        drop(reservation);
        let connected = connect(offer, bind, &stop).await.unwrap();
        assert!(UdpSocket::bind(bind).await.is_err());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let browser = listener.local_addr().unwrap();
        drop(connected.serve(
            listener,
            format!("thepeer-{}.localhost", "b".repeat(32)),
            &stop,
        ));
        loop {
            if UdpSocket::bind(bind).await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(tokio::net::TcpStream::connect(browser).await.is_err());
        assert!(
            timeout(Duration::from_millis(100), target.accept())
                .await
                .is_err()
        );
        helper.shutdown().await.unwrap();
        assert!(!stop.is_cancelled());
    })
    .await
    .expect("bounded unpolled-drop fixture");
}

#[tokio::test]
async fn invalid_helper_participants_are_rejected_before_observer_traffic() {
    let observer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let stop = CancellationToken::new();
    let config = HelperConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        target: "localhost:3002".parse().unwrap(),
        observer: Some(observer.local_addr().unwrap()),
    };
    let result = Helper::prepare_with_relay(config, "127.0.0.1:0".parse().unwrap(), &stop).await;
    assert!(matches!(result, Err(Failure("route-input"))));
    assert!(observer.try_recv_from(&mut [0; 2048]).is_err());
    let result = Helper::prepare(
        HelperConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            target: "localhost:3002".parse().unwrap(),
            observer: Some("[::1]:10000".parse().unwrap()),
        },
        &stop,
    )
    .await;
    assert!(matches!(result, Err(Failure("route-family"))));
}
