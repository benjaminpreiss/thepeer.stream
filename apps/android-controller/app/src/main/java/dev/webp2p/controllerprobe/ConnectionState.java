package dev.webp2p.controllerprobe;

/** Process-local connection presentation and one-shot browser launch. Never retains a grant.
 * Shared by the foreground owner and replacement Activities; nothing is persisted. */
final class ConnectionState {
    private boolean active, stopping, browserAttempted;
    private String hostname = "";
    private String message = "Connect to scan your trusted computer's application QR.";

    static final class Snapshot {
        final boolean active, ready, stopping;
        final String message, details, url;
        Snapshot(boolean active, boolean ready, boolean stopping, String message, String details, String url) {
            this.active = active; this.ready = ready; this.stopping = stopping;
            this.message = message; this.details = details; this.url = url;
        }
    }

    synchronized boolean begin(ViewerLaunch launch, String host) {
        if (active || launch == null) return false;
        active = true; stopping = false; browserAttempted = false;
        hostname = host;
        message = "Connecting and verifying the computer…";
        return true;
    }

    synchronized Snapshot snapshot(int localPort) { return snapshot(localPort, 0); }

    synchronized Snapshot snapshot(int localPort, int routeKind) {
        boolean ready = active && !stopping && localPort > 0;
        String url = ready ? "http://" + hostname + ":" + localPort + "/" : "";
        String route = routeKind == 1 ? "Direct"
                : routeKind == 3 ? "Single-carrier relay" : "Unavailable";
        String details = ready ? "Route: " + route + "\nComputer identity and application grant verified."
                + "\n\nApp address:\n" + url : "";
        return new Snapshot(active, ready, stopping, ready ? "Connected" : message, details, url);
    }

    /** Consume before launching the Intent, even if no browser handles it. Manual Open remains. */
    synchronized boolean takeBrowserLaunch(int localPort) {
        if (!snapshot(localPort).ready || browserAttempted) return false;
        browserAttempted = true;
        return true;
    }

    synchronized void stop() {
        if (!active) return;
        stopping = true; browserAttempted = true;
        message = "Stopping…";
    }

    synchronized void finish(String result) {
        active = false; stopping = false; browserAttempted = true;
        hostname = "";
        message = result;
    }
}
