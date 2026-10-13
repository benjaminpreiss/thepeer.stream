use super::*;

fn args(values: &[&str]) -> Arguments {
    arguments(&values.iter().map(|v| (*v).to_owned()).collect::<Vec<_>>()).unwrap()
}

#[test]
fn bind_is_optional_but_target_is_required() {
    assert!(args(&["--target-port", "3002"]).bind.is_none());
    assert!(arguments(&[]).is_err());
    assert!(arguments(&["--target-port".into(), "0".into()]).is_err());
}

#[test]
fn explicit_bind_is_preserved_without_discovery() {
    let (config, relay) = args(&[
        "--target-port",
        "3002",
        "--bind",
        "127.0.0.1:43210",
        "--relay",
        "192.0.2.1:10000",
    ])
    .resolve()
    .unwrap();
    assert_eq!(config.bind, "127.0.0.1:43210".parse().unwrap());
    assert_eq!(relay, Some("192.0.2.1:10000".parse().unwrap()));
}

#[test]
fn participant_route_discovers_a_local_ip_without_sending_a_datagram() {
    for destination in ["127.0.0.1:0", "[::1]:0"] {
        let peer = UdpSocket::bind(destination).unwrap();
        peer.set_nonblocking(true).unwrap();
        let destination = peer.local_addr().unwrap();
        let (config, relay) = args(&["--target-port", "3002", "--relay", &destination.to_string()])
            .resolve()
            .unwrap();
        assert_eq!(config.bind.ip(), destination.ip());
        assert_eq!(config.bind.port(), 0, "lookup port must not be advertised");
        assert_eq!(relay, Some(destination));
        assert_eq!(
            peer.recv_from(&mut [0; 2048]).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        // The result is an explicit, bindable address, not a wildcard.
        let actual = UdpSocket::bind(config.bind).unwrap();
        assert_eq!(actual.local_addr().unwrap().ip(), destination.ip());
    }
}

#[test]
fn mixed_participant_families_fail_before_discovery() {
    assert!(matches!(
        args(&[
            "--target-port",
            "3002",
            "--observer",
            "127.0.0.1:10000",
            "--relay",
            "[::1]:10000",
        ])
        .resolve(),
        Err(Failure("route-family"))
    ));
}
