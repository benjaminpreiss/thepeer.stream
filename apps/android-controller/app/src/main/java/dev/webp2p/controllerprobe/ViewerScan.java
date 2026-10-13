package dev.webp2p.controllerprobe;

/** One explicitly opened scan, on the UI thread. No intents, URLs or network actions. */
final class ViewerScan {
    static final long LIFETIME_MS = 60_000;
    private final long started;
    private boolean active = true;
    ViewerScan(long now) { started = now; }
    void cancel() { active = false; }
    /** Cancelled/expired/duplicate results are ignored; malformed first scans consume the scan. */
    ViewerLaunch accept(String raw, long now) {
        if (!active) return null;
        active = false;
        if (now < started || now - started >= LIFETIME_MS) return null;
        return ViewerLaunch.parse(raw);
    }
    @Override public String toString() { return "ViewerScan[redacted]"; }
}
