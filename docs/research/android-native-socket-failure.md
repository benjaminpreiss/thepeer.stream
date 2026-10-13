# Android native socket failure: permissions, LineageOS policy and next checks

## Conclusion

We have **not established that native UDP is inherently unavailable on this phone**.
The prior APK failed at socket creation with ECONNREFUSED, but the ADB-shell native
probe and browser WebRTC worked; a different, later APK successfully created a TCP
loopback listener. The follow-up experiment passed UDP creation in that **same current APK/process**,
alongside its working TCP server. A subsequent phone-local round-trip also passed;
a subsequent bounded IPv4 Wi-Fi echo also passed with one accepted/echoed packet
and no rejected sources. The direct pinned native Saorsa/QUIC echo subsequently
passed too, with user-confirmed wrong-pin rejection. Native MASQUE echo v2 also
passed on the phone, with verified echo and positive bidirectional relay counters.
Bounded native HTTP inspection subsequently passed on the phone as well. The new
HTML/CSS-only native viewer is desktop-tested; its foreground/browser phone gate
is pending.

The earlier inference that `restricted_networking_mode=1` alone explained the
failure was too strong. Inspected LineageOS 23.0 policy code differs materially
from stock AOSP restricted-mode logic: its restricted-mode allowlist also considers
per-UID `POLICY_REJECT_ALL`. Ordinary apps are not necessarily blocked just because
the global flag is on. This is a promising per-app-policy explanation, not a
confirmed diagnosis of the old installed APK.

The research itself performed no device commands, settings changes, privileged
permission grants, native-client restoration or network deployment. No
background-agent tool was available; sources were inspected directly.

Follow-up: the user approved a creation-only test. The current APK now has a
**Test TCP / UDP socket creation** button implementing steps 1–2 below, with
separate creation/closure results. The phone passed all four cases (API 36, target
35, INTERNET granted) without policy changes. The user subsequently authorized a
loopback request/echo test, which passed for IPv4 and IPv6 on the phone. A bounded
phone/computer IPv4 LAN echo test was then authorized, implemented and passed on
the phone; the helper reported accepted=1, echoed=1, rejected=0. A minimal native
Saorsa transport test in the current APK was built by the user outside the agent
sandbox and passed on the phone (admitted=1, echoed=1, failed=0, rejected=0), with
subsequent user confirmation of wrong-pin rejection. A separately authorized
native MASQUE echo fixture also passed on the phone: admitted=1, echoed=1, failed=0,
rejected=0; relay → echo 15 packets / 4,425 bytes, reverse 13 / 7,604, zero rejected
sources. An earlier zero-traffic handshake failure remains unexplained. No sandbox
expansion or device-policy changes were needed. See the
[operator steps](../../apps/android-controller/README.md#lan-udp-echo-diagnostic--phone-passed).

## What the project actually recorded

Historical records in `docs/STATUS.md` say:

- Phone: LineageOS, Android 16 / API 36; old APK targeted SDK 37.
- INTERNET permission was reported granted; RESTRICT_LOCAL_NETWORK compatibility
  change was reported disabled. Always-on VPN was absent, but ordinary VPN/firewall
  state was not fully established.
- Android `Os.socket` and split Rust creation/bind checks both failed **at socket
  creation**, before bind, with ECONNREFUSED (111). This is not simply QUIC handshake
  failure, remote listener refusal, NAT, or a Rust-only error.
- The ADB-shell CLI reached the test listener (expected wrong-pin rejection), and
  the browser subsequently passed WebRTC/PQ tests. Those are different security
  identities/contexts from the old APK.
- The new `dev.webp2p.controllerprobe` APK (target SDK 35) successfully serves TCP
  loopback HTTP. Its current manifest declares INTERNET. Follow-up: it has now
  passed explicit IPv4/IPv6 UDP creation/closure and local round-trip on the phone;
  the bounded IPv4 Wi-Fi echo and direct pinned native Saorsa echo have also
  passed. Native MASQUE echo and bounded native HTTP inspection subsequently
  passed on the phone too. The HTML/CSS-only native viewer is the next, separately
  authorized, pending phone gate.

These are historical user observations, not a fresh device inspection. The exact
ROM build and policy state must be checked before attributing a specific cause.
No phone addresses, serial numbers or session information are retained here.

## 1. Did we omit a basic permission?

For an ordinary **unicast** UDP socket, INTERNET is the basic manifest permission.
There is no separate generic UDP permission. It is a normal/install-time permission,
not a runtime dialog. The other permissions in the supplied search summary do not
substitute for it [1]:

| Permission | Purpose | Does omission explain plain UDP socket creation failure? |
| --- | --- | --- |
| INTERNET | Open network sockets | Can, but the old run recorded it granted and the current manifest includes it |
| ACCESS_NETWORK_STATE | Read connectivity information | Not required just to call `Os.socket`/`DatagramSocket` |
| ACCESS_WIFI_STATE | Read Wi-Fi information | Not required for ordinary unicast socket creation |
| CHANGE_WIFI_MULTICAST_STATE | Enter Wi-Fi multicast mode | Relevant to multicast filtering/locks, not our explicit unicast relay destination |

Network operations should run off the UI thread. A normal main-thread networking
violation is a separate issue; adding background dispatch does not grant a UID
network access. A foreground service also does not override firewall/permission
policy or guarantee unrestricted network use during all power states.

Additional local-network permissions depend on OS version and target SDK; see
section 4. Do not add every permission speculatively or request privileged network
permissions to repair an unconfirmed diagnosis.

## 2. Why ECONNREFUSED can mean a local policy denial

LineageOS `NetdClient.cpp::netdClientSocket` [2] calls the underlying socket function.
If creation of an AF_INET/AF_INET6 socket fails with EPERM, it deliberately changes
the errno to ECONNREFUSED to avoid exposing a Java SecurityException. Its comment
explicitly notes that ECONNREFUSED is not normally a socket-creation result.

There is also a separate process-level `allowNetworkingForProcess` early check
that returns EPERM directly; not every denial takes the remapping path. The errno
alone does not identify which firewall/permission rule denied access.

The inspected Connectivity BPF program's `inet_socket_create` [3] checks the app's
INTERNET permission map and UID owner-firewall rules. That function does **not**
branch on TCP versus UDP or inspect a remote destination. It can therefore deny
socket creation before there is a target route/port to contact. The owner-firewall
function passes system UIDs separately, helping explain why an ADB-shell probe is
not equivalent to a normal APK probe.

Consequences:
- Changing a destination port, binding a different address, switching Rust to Java,
  or changing QUIC framing cannot fix this specific earlier-stage denial.
- TCP succeeding in the **same UID, process and policy state** would be significant
  evidence against this blanket creation denial. Different APKs/dates do not prove
  the same policy was applied.
- Creating sockets through raw syscalls is not an appropriate workaround: kernel
  policy remains authoritative. No root/SELinux/firewall bypass is proposed.

## 3. Important LineageOS-specific policy distinction

Stock AOSP Android 16 computes restricted-mode access from privileged permissions
in `updateBlockedReasonsForRestrictedModeUL` [4]. The inspected LineageOS method
instead computes [5]:

```java
final boolean isBlockedOnAllNetworks = isUidBlockedOnAllNetworks(uid);
final boolean hasRestrictedModeAccess = hasRestrictedModeAccess(uid)
        || !isBlockedOnAllNetworks;
```

Its `isUidBlockedOnAllNetworks` checks `POLICY_REJECT_ALL` (0x40000), and the method
keeps a UID blocked on all networks out of the restricted-mode allowlist. This
means the global flag alone is **not** enough to conclude ordinary apps need a
privileged exception on this ROM.

LineageOS Settings connects the per-app `restrict_all` switch to that policy [6].
Its English label is **Allow network access**, with separate **Wi-Fi data** and
mobile/VPN controls [7]. The checked UI boolean is inverted when setting rejection
policy; inspecting the UI should follow its visible label rather than the internal
`restrict_all` name.

A normal INTERNET manifest grant and a per-app network denial are separate facts.
The source also adds POLICY_REJECT_ALL for non-system packages lacking INTERNET
on package-added handling [5]. An old install/update history is therefore worth
checking, but we have no evidence that this happened to the old APK.

The protected CONNECTIVITY_USE_RESTRICTED_NETWORKS permission is
`signature|privileged` [1]. Adding it to an ordinary manifest is not a general
solution. Do not turn off global restricted networking as a blanket workaround.

## 4. Android local-network restrictions are another, distinct layer

Current Android documentation [8] distinguishes:

- Android 16: local-network protection was an opt-in compatibility change.
- Android 17 / target SDK 37+: local access uses ACCESS_LOCAL_NETWORK runtime
  authorization under that platform's enforcement rules.

The old record says Android 16 and the opt-in restriction was disabled. Its SDK 37
target is a variable to track, not proof that Android 17 enforcement was active on
an Android 16 device. Local destination/send/receive denials must also be separated
from failure to create a socket before a destination exists.

For future target/OS upgrades, implement the documented user permission flow when
applicable; do not disable compatibility checks to hide a missing permission.

## 5. Ranked explanations worth testing

1. **Old APK's per-UID network policy** (not just its manifest permission), such as
   Allow network access/Wi-Fi data restrictions. Prediction: old and current APKs
   show different policy or current APK UDP succeeds despite the old result.
2. **Another UID/process enforcement state**, including power-state/firewall/VPN
   conditions or a stale/mismatched kernel permission map. Prediction: failure
   correlates with the same UID's TCP creation or foreground/policy state; a real
   packet/send failure may instead occur only after creation succeeds.
3. **OS/target/compatibility changes between experiments.** Prediction: permission
   and denial stage correlate with the actual build/compat state, not library choice.
4. **Family/protocol-specific platform defect or mismatch.** Lower confidence until
   the same-process IPv4/IPv6 TCP/UDP matrix is measured. Both Rust and Android
   creation previously failing makes a Rust-only explanation weak.

Remote NAT, relay pin errors, multicast locks and HTML cleartext configuration do
not explain a confirmed bare unicast socket-creation refusal.

## 6. Supported next step and conditional remediation

Do **not** resurrect the old native client first. Use a small opt-in diagnostic
in the current working Android app, off the UI thread (steps 1–2 implemented):

1. Record only OS/API, target SDK, app UID, INTERNET grant and exact failure stage.
2. Create and close AF_INET TCP and UDP sockets, then AF_INET6 equivalents. No
   packets, no remote addresses, no policy changes. Use `android.system.Os.socket`
   so creation is separate from bind/connect.
3. If UDP creation succeeds, separately test numeric-loopback bind and close.
4. Only with further explicit approval, use a bounded LAN echo to the existing
   computer fixture, then test pinned Saorsa transport. Keep destination, packet
   count/size and duration bounded. No public relay is implied.

Read-only context checks may include:

```sh
adb shell settings get global restricted_networking_mode
adb shell cmd package list packages -U dev.webp2p.controllerprobe
```

Do not share complete `dumpsys netpolicy` or device dumps: they may contain other
apps, network/account identifiers and private state. Inspect only the relevant
UID/permission/policy locally, or use a deliberately redacted diagnostic.

If a user-owned per-app network setting is confirmed to be the cause, its ordinary
Settings permission/control is the supported remediation **only with explicit user
approval**. An administrator-enforced block must be respected. Preserve the global
policy and do not use root, privileged-app installation, SELinux changes, raw-syscall
tricks or broad firewall exceptions. If current UDP already works, no setting
change is needed at all.

If native UDP and the pinned transport work in the current app, the native app
could eventually own the tunnel and serve target HTTP locally to a normal browser
tab. That would remove browser document-lifetime constraints, but remains a future
architecture change requiring separate approval/testing—not a result of this research.

## Sources and version boundaries

LineageOS branch `lineage-23.0` was examined because the old record says Android 16;
these repository revisions are **not confirmed to match the installed ROM build**.
Pinned NetdClient and policy files were byte-compared with the inspected branch
files. Public Android docs describe current platform behavior, not this device's
measured state.

[1] Platform manifest permission definitions (INTERNET/network/Wi-Fi/multicast and
restricted-networks privilege):
<https://github.com/LineageOS/android_frameworks_base/blob/155701ba6a05d0061dbab1b9f1bfec2040cf4158/core/res/AndroidManifest.xml>.

[2] Socket errno remapping:
<https://github.com/LineageOS/android_system_netd/blob/4b8e3002bddbe11bab4e240cfa13efe66a50d8d5/client/NetdClient.cpp#L181-L204>.

[3] UID/firewall and socket-creation BPF checks:
<https://github.com/LineageOS/android_packages_modules_Connectivity/blob/b73770f9c16e3daa3a36966d9fe17a6d4fee6f42/bpf/progs/netd.c#L463-L475>
and <https://github.com/LineageOS/android_packages_modules_Connectivity/blob/b73770f9c16e3daa3a36966d9fe17a6d4fee6f42/bpf/progs/netd.c#L773-L795>.

[4] AOSP comparison:
<https://github.com/aosp-mirror/platform_frameworks_base/blob/android16-release/services/core/java/com/android/server/net/NetworkPolicyManagerService.java>.

[5] LineageOS per-UID restricted-mode policy and package-added handling:
<https://github.com/LineageOS/android_frameworks_base/blob/155701ba6a05d0061dbab1b9f1bfec2040cf4158/services/core/java/com/android/server/net/NetworkPolicyManagerService.java#L4959-L5039>
and <https://github.com/LineageOS/android_frameworks_base/blob/155701ba6a05d0061dbab1b9f1bfec2040cf4158/services/core/java/com/android/server/net/NetworkPolicyManagerService.java#L1483-L1496>.

[6] Settings switch behavior:
<https://github.com/LineageOS/android_packages_apps_Settings/blob/ff1e876d2e558bc72cb51fb9d8206115f8ef75f9/src/com/android/settings/datausage/AppDataUsage.java>.

[7] User-visible setting labels:
<https://github.com/LineageOS/android_packages_apps_Settings/blob/ff1e876d2e558bc72cb51fb9d8206115f8ef75f9/res/values/cm_strings.xml#L125-L132>.

[8] Android local-network permissions:
<https://developer.android.com/privacy-and-security/local-network-permission>.

[9] Android networking guidance (normal/install-time permissions):
<https://developer.android.com/develop/connectivity/network-ops/connecting>.
