package dev.webp2p.controllerprobe;

/** In-memory, single-owner transfer. Intents carry only the non-secret generation. */
final class ConnectionHandoff {
    static final class Arguments implements AutoCloseable {
        final long generation;
        final ViewerLaunch launch;
        final String hostname;
        Arguments(long generation, ViewerLaunch launch, String hostname) {
            this.generation = generation; this.launch = launch; this.hostname = hostname;
        }
        @Override public void close() { launch.close(); }
    }
    private long next, active;
    private boolean claimed;
    private Arguments pending;

    synchronized long offer(ViewerLaunch launch, String hostname) {
        if (active != 0 || next == Long.MAX_VALUE) { launch.close(); return 0; }
        active = ++next;
        pending = new Arguments(active, launch, hostname);
        return active;
    }
    synchronized Arguments claim(long generation) {
        if (generation != active || pending == null || claimed) return null;
        Arguments arguments = pending;
        pending = null;
        claimed = true;
        return arguments;
    }
    synchronized boolean cancelPending() {
        if (pending == null || claimed) return false;
        pending.close(); pending = null; active = 0;
        return true;
    }
    synchronized boolean owns(long generation) {
        return generation != 0 && generation == active && claimed;
    }
    synchronized boolean release(long generation) {
        if (!owns(generation)) return false;
        active = 0; claimed = false;
        return true;
    }
}
