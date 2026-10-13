use super::{Attempt, Candidates, InvalidAddress, plan};
use std::net::{SocketAddr, SocketAddrV6};

fn addr(text: &str) -> SocketAddr {
    text.parse().expect("fixture address")
}
fn candidates(bound: &str, observed: Option<&str>, relay: Option<&str>) -> Candidates {
    Candidates {
        bound: addr(bound),
        observed: observed.map(addr),
        relay: relay.map(addr),
    }
}
fn direct(text: &str) -> Attempt {
    Attempt::Direct(addr(text))
}
fn relay(text: &str) -> Attempt {
    Attempt::Relay(addr(text))
}
fn attempts(candidates: Candidates, own: Option<&str>) -> Vec<Attempt> {
    plan(candidates, own.map(addr))
        .unwrap()
        .attempts()
        .collect()
}

#[test]
fn shared_wan_prefers_lan_then_public_direct_then_relay() {
    let input = candidates(
        "192.168.2.120:5000",
        Some("198.51.100.7:6000"),
        Some("203.0.113.8:7000"),
    );
    // The phone's NAT port is unrelated to the helper's mapped port.
    assert_eq!(
        attempts(input, Some("198.51.100.7:9000")),
        vec![
            direct("192.168.2.120:5000"),
            direct("198.51.100.7:6000"),
            relay("203.0.113.8:7000"),
        ]
    );
}

#[test]
fn different_or_unknown_wan_does_not_speculatively_dial_private_addresses() {
    let input = candidates(
        "192.168.2.120:5000",
        Some("198.51.100.7:6000"),
        Some("203.0.113.8:7000"),
    );
    for own in [None, Some("198.51.100.9:9000"), Some("192.168.2.22:9000")] {
        assert_eq!(
            attempts(input, own),
            vec![direct("198.51.100.7:6000"), relay("203.0.113.8:7000")]
        );
    }
}

#[test]
fn local_only_invitation_needs_no_public_bootstrap() {
    for bound in ["192.168.2.120:5000", "127.0.0.1:5000", "[fd00::1]:5000"] {
        assert_eq!(
            attempts(candidates(bound, None, None), None),
            vec![direct(bound)]
        );
    }
}

#[test]
fn relay_address_is_not_a_wan_observation_and_is_not_removed_on_same_wan() {
    assert_eq!(
        attempts(
            candidates("192.168.2.120:5000", None, Some("198.51.100.7:7000")),
            Some("198.51.100.7:9000")
        ),
        vec![relay("198.51.100.7:7000")]
    );
    assert_eq!(
        attempts(
            candidates(
                "192.168.2.120:5000",
                Some("198.51.100.7:6000"),
                Some("198.51.100.7:7000")
            ),
            Some("198.51.100.7:9000")
        ),
        vec![
            direct("192.168.2.120:5000"),
            direct("198.51.100.7:6000"),
            relay("198.51.100.7:7000")
        ]
    );
}

#[test]
fn deduplicates_direct_but_preserves_relay_provenance() {
    assert_eq!(
        attempts(
            candidates(
                "198.51.100.7:5000",
                Some("198.51.100.7:5000"),
                Some("198.51.100.7:5000")
            ),
            None
        ),
        vec![direct("198.51.100.7:5000"), relay("198.51.100.7:5000")]
    );
}

#[test]
fn cgnat_and_private_observations_do_not_establish_shared_public_wan() {
    for observed in [
        "100.64.0.1:6000",
        "100.127.255.254:6000",
        "10.2.3.4:6000",
        "169.254.1.2:6000",
        "[fd00::2]:6000",
        "[::1]:6000",
    ] {
        assert_eq!(
            attempts(
                candidates(
                    "192.168.2.120:5000",
                    Some(observed),
                    Some("203.0.113.8:7000")
                ),
                Some(observed)
            ),
            vec![relay("203.0.113.8:7000")]
        );
    }
}

#[test]
fn globally_addressed_bound_socket_is_direct_without_a_matching_observer() {
    assert_eq!(
        attempts(
            candidates(
                "198.51.100.7:5000",
                Some("198.51.100.8:6000"),
                Some("203.0.113.8:7000")
            ),
            None
        ),
        vec![
            direct("198.51.100.7:5000"),
            direct("198.51.100.8:6000"),
            relay("203.0.113.8:7000")
        ]
    );
}

#[test]
fn ipv6_plan_retains_addresses_without_synthesizing_or_switching_families() {
    assert_eq!(
        attempts(
            candidates(
                "[fd00::1]:5000",
                Some("[2001:db8::1]:6000"),
                Some("[2001:db8::2]:7000")
            ),
            Some("[2001:db8::1]:9000")
        ),
        vec![
            direct("[fd00::1]:5000"),
            direct("[2001:db8::1]:6000"),
            relay("[2001:db8::2]:7000")
        ]
    );
    // Planning is not family eligibility: the executing adapter must reject a
    // selected incompatible family, while unused valid hints need not fail.
    assert_eq!(
        attempts(
            candidates("198.51.100.7:5000", None, Some("[2001:db8::2]:7000")),
            None
        ),
        vec![direct("198.51.100.7:5000"), relay("[2001:db8::2]:7000")]
    );
}

#[test]
fn invalid_candidates_and_context_fail_before_any_plan_even_when_unused() {
    let good = candidates("198.51.100.7:5000", None, None);
    let mut invalid = vec![
        addr("0.0.0.0:5000"),
        addr("198.51.100.7:0"),
        addr("224.0.0.1:5000"),
        addr("255.255.255.255:5000"),
        addr("[::]:5000"),
        addr("[ff02::1]:5000"),
        addr("[::ffff:192.168.2.120]:5000"),
        addr("[fe80::1]:5000"),
    ];
    for (flow, scope) in [(1, 0), (0, 1)] {
        invalid.push(SocketAddr::V6(SocketAddrV6::new(
            "2001:db8::1".parse().unwrap(),
            5000,
            flow,
            scope,
        )));
    }
    for bad in invalid {
        for input in [
            Candidates { bound: bad, ..good },
            Candidates {
                observed: Some(bad),
                ..good
            },
            Candidates {
                relay: Some(bad),
                ..good
            },
        ] {
            assert_eq!(plan(input, None), Err(InvalidAddress));
        }
        assert_eq!(plan(good, Some(bad)), Err(InvalidAddress));
    }
}
