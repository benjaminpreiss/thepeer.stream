package dev.webp2p.controllerprobe;

import java.util.Arrays;

public final class ConnectionHandoffTest {
    private static int checks;
    private static void check(boolean value) { checks++; if (!value) throw new AssertionError("handoff " + checks); }
    private static ViewerLaunch launch() {
        return ViewerLaunchTest.launch();
    }
    private static void closed(ViewerLaunch launch) {
        try { launch.takeOffer(); throw new AssertionError("Discarded packet survived"); }
        catch (IllegalStateException expected) { check(true); }
    }
    public static void main(String[] args) {
        ConnectionHandoff handoff = new ConnectionHandoff();
        String host = "thepeer-00112233445566778899aabbccddeeff.localhost";
        ViewerLaunch first = launch();
        long one = handoff.offer(first, host);
        check(one != 0 && !handoff.owns(one));
        check(handoff.claim(0) == null && handoff.claim(one + 1) == null);
        ViewerLaunch duplicate = launch(); check(handoff.offer(duplicate, host) == 0); closed(duplicate);
        check(handoff.cancelPending()); closed(first); // Stop or failed framework startup, before service delivery.
        check(!handoff.cancelPending() && handoff.claim(one) == null);

        ViewerLaunch second = launch();
        long two = handoff.offer(second, host);
        check(two > one);
        check(handoff.claim(one) == null); // Stale queued start cannot consume a replacement.
        check(!handoff.release(one)); // Stale service destruction cannot clear it either.
        ConnectionHandoff.Arguments claimed = handoff.claim(two);
        check(claimed != null && claimed.launch == second && claimed.hostname.equals(host));
        check(handoff.owns(two) && !handoff.owns(one) && !handoff.owns(0));
        check(handoff.claim(two) == null && !handoff.cancelPending());
        ViewerLaunch busy = launch(); check(handoff.offer(busy, host) == 0); closed(busy);
        claimed.close(); closed(second); // Worker construction/start failed before JNI transfer.
        check(handoff.owns(two)); // Must still await service + worker cleanup, not release early.
        check(!handoff.release(one) && handoff.release(two) && !handoff.release(two));

        ViewerLaunch third = launch(); long three = handoff.offer(third, host);
        check(three > two && !handoff.release(two));
        ConnectionHandoff.Arguments running = handoff.claim(three);
        byte[] packet = running.launch.takeOffer();
        check(Arrays.equals(packet, ViewerLaunchTest.offer().getBytes(java.nio.charset.StandardCharsets.US_ASCII)));
        running.close(); closed(third); // JNI, not the emptied offer, now owns this array.
        check(!handoff.cancelPending() && handoff.owns(three));
        Arrays.fill(packet, (byte) 0); // Simulated caller finally; actual wiping has separate real JNI tests.
        check(handoff.release(three) && !handoff.owns(three));
        System.out.println("PASS: " + checks + " handoff checks: pending cancellation, stale delivery/destruction, single transfer and worker-failure cleanup (not Android service evidence).");
    }
}
