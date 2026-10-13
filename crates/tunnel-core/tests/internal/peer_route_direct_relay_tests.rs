use super::*;
use tokio::net::UdpSocket;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn direct_silence_is_unavailable_and_releases_the_phone() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let stop = CancellationToken::new();
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (helper, mut offer) = Helper::prepare(
            HelperConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                target_port: target.local_addr().unwrap().port(),
                observer: None,
            },
            &stop,
        )
        .await
        .unwrap();
        let quiet = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        offer.direct = quiet.local_addr().unwrap();
        offer.bound = Some(offer.direct);
        let reserve = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let phone = reserve.local_addr().unwrap();
        drop(reserve);
        let result = connect(offer, phone, &stop).await;
        assert_eq!(result.err().map(|e| e.0), Some("route-unavailable"));
        assert!(
            quiet.try_recv_from(&mut [0; 2048]).is_ok(),
            "Direct was actually attempted"
        );
        assert!(
            timeout(Duration::from_millis(50), target.accept())
                .await
                .is_err()
        );
        let _released = UdpSocket::bind(phone).await.unwrap();
        helper.shutdown().await.unwrap();
    })
    .await
    .expect("bounded Direct silence check");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn production_observer_stop_and_drop_release_the_phone_without_authorizing_target() {
    timeout(Duration::from_secs(35), async {
        for drop_future in [false, true] {
            let helper_stop = CancellationToken::new();
            let phone_stop = CancellationToken::new();
            let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let (helper, mut offer) = Helper::prepare(
                HelperConfig {
                    bind: "127.0.0.1:0".parse().unwrap(),
                    target_port: target.local_addr().unwrap().port(),
                    observer: None,
                },
                &helper_stop,
            )
            .await
            .unwrap();
            let observer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            offer.observer = Some(observer.local_addr().unwrap());
            let reserve = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let phone = reserve.local_addr().unwrap();
            drop(reserve);
            let task_stop = phone_stop.clone();
            let connecting = tokio::spawn(async move { connect(offer, phone, &task_stop).await });
            let (_, source) = timeout(Duration::from_secs(5), observer.recv_from(&mut [0; 2048]))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(source, phone, "real observer bootstrap packet");
            if drop_future {
                connecting.abort();
                assert!(matches!(connecting.await, Err(error) if error.is_cancelled()));
            } else {
                phone_stop.cancel();
                let result = timeout(Duration::from_secs(15), connecting)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(result.err().map(|e| e.0), Some("peer-cancelled"));
            }
            timeout(Duration::from_secs(10), async {
                loop {
                    if UdpSocket::bind(phone).await.is_ok() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .expect("observer runtime must release UDP");
            assert!(
                timeout(Duration::from_millis(50), target.accept())
                    .await
                    .is_err()
            );
            helper.shutdown().await.unwrap();
        }
    })
    .await
    .expect("bounded observer ownership check");
}
