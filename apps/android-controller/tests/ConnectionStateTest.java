package dev.webp2p.controllerprobe;

/** Exercises the production state shared by the Activity and foreground owner. Not Android UI evidence. */
public final class ConnectionStateTest {
    private static int checks;
    private static void check(boolean condition) {
        checks++;
        if (!condition) throw new AssertionError("Connection state assertion " + checks);
    }
    public static void main(String[] args) {
        ConnectionState state = new ConnectionState();
        ViewerLaunch app = ViewerLaunchTest.launch();
        String host = "thepeer-00112233445566778899aabbccddeeff.localhost";
        check(!state.snapshot(0).active);
        check(!state.begin(null, host));
        check(state.begin(app, host));
        check(!state.begin(app, host));
        check(state.snapshot(0).active && !state.snapshot(0).ready);
        check(!state.takeBrowserLaunch(0));
        ConnectionState.Snapshot ready = state.snapshot(1234, 1);
        check(ready.ready && !ready.stopping);
        check(ready.url.equals("http://" + host + ":1234/"));
        check(ready.details.contains("Route: Direct"));
        check(state.snapshot(1234, 2).details.contains("Unavailable"));
        check(state.snapshot(1234, 3).details.contains("Single-carrier relay"));
        check(!ready.details.contains(ViewerLaunchTest.offer()) && !ready.url.contains("wpa2:"));
        app.close(); // Presentation retains neither the launch nor its capability.
        check(state.takeBrowserLaunch(1234));
        // Returning from the browser, rotation, a cancelled chooser or a failed Intent
        // reuse this state and must not launch again. Open app still has a ready URL.
        for (int i = 0; i < 10; i++) check(!state.takeBrowserLaunch(1234));
        check(state.snapshot(1234).ready);
        state.stop();
        check(state.snapshot(1234).stopping && !state.snapshot(1234).ready);
        check(state.snapshot(1234).url.isEmpty());
        check(!state.takeBrowserLaunch(1234) && !state.begin(app, host));
        state.finish("Stopped");
        check(!state.snapshot(1234).active && !state.snapshot(1234).ready);
        check(state.snapshot(1234).details.isEmpty());
        check(state.begin(app, host));
        state.stop(); // Stop before ready also suppresses auto-open.
        check(!state.takeBrowserLaunch(1234));
        state.finish("Connection failed");
        check(state.begin(app, host));
        check(state.takeBrowserLaunch(1234));
        state.finish("Disconnected");
        check(state.snapshot(0).message.equals("Disconnected"));
        System.out.println("PASS: " + checks + " connection-state checks; one-shot browser launch, reopening, Stop, restart and grant-free presentation. Not Android UI evidence.");
    }
}
