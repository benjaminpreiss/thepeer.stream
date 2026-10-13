use super::*;

fn args(values: &[&str]) -> Arguments {
    arguments(&values.iter().map(|v| (*v).to_owned()).collect::<Vec<_>>()).unwrap()
}

#[test]
fn bind_is_optional_but_target_is_required() {
    assert!(args(&["--target", "localhost:3002"]).bind.is_none());
    assert!(arguments(&[]).is_err());
    assert!(arguments(&["--target".into(), "localhost:0".into()]).is_err());
}

#[test]
fn target_is_a_host_and_port_not_a_url_or_nonlocal_literal() {
    for authority in [
        "localhost:4321",
        "127.0.0.1:4321",
        "[::1]:4321",
        "myapp.test:4321",
    ] {
        // Hostname loopback validation belongs to preparation, after resolution.
        let parsed = args(&["--target", authority]);
        assert_eq!(parsed.target.port(), 4321);
        assert_eq!(parsed.target.authority(), authority);
    }
    for value in [
        "4321",
        "localhost",
        "localhost:0",
        "localhost:65536",
        "localhost:+4321",
        "localhost: 4321",
        "localhost:4321/",
        "http://localhost:4321",
        "localhost:4321?x=1",
        "localhost:4321#fragment",
        "user@localhost:4321",
        "192.168.1.2:4321",
        "0.0.0.0:4321",
        "[::]:4321",
        "localhost.:4321",
        " localhost:4321",
    ] {
        assert!(
            arguments(&["--target".into(), value.into()]).is_err(),
            "accepted invalid target"
        );
    }
    assert!(
        arguments(&[
            "--target".into(),
            "localhost:4321".into(),
            "--target".into(),
            "localhost:4322".into()
        ])
        .is_err()
    );
    assert!(arguments(&["--target-port".into(), "4321".into()]).is_err());
}

#[test]
fn explicit_bind_is_preserved_without_discovery() {
    let (config, relay) = args(&[
        "--target",
        "localhost:3002",
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
        let (config, relay) = args(&[
            "--target",
            "localhost:3002",
            "--relay",
            &destination.to_string(),
        ])
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
            "--target",
            "localhost:3002",
            "--observer",
            "127.0.0.1:10000",
            "--relay",
            "[::1]:10000",
        ])
        .resolve(),
        Err(Failure("route-family"))
    ));
}
