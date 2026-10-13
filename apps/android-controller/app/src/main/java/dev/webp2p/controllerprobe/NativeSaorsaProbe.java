package dev.webp2p.controllerprobe;

/** Target-only JNI owner. Missing/incompatible native builds fail closed. */
final class NativeSaorsaProbe {
    private static volatile boolean loaded;
    private static void load() {
        if (!loaded) { System.loadLibrary("controller_saorsa_probe"); loaded = true; }
    }
    static synchronized boolean routingSupported() {
        try { load(); return nativeRouteSupported(); }
        catch (LinkageError | RuntimeException error) { return false; }
    }
    static synchronized boolean prepareViewer() {
        try { load(); return nativeViewerPrepare(); }
        catch (LinkageError | RuntimeException error) { return false; }
    }
    /** Takes ownership even on JNI failure. No capability in diagnostics or Intents. */
    static String runRouting(byte[] offer, String hostname) {
        try { return nativeRouteRun(offer, hostname); }
        catch (LinkageError | RuntimeException error) {
            return "Routing native runtime/library unavailable. Rebuild and fully restart the app.";
        } finally { if (offer != null) java.util.Arrays.fill(offer, (byte) 0); }
    }
    static int routeKind() {
        if (loaded) try { return nativeRouteKind(); } catch (LinkageError | RuntimeException ignored) { }
        return 0;
    }
    static void abandonPreparedViewer() {
        if (loaded) try { nativeViewerAbandonPrepared(); } catch (LinkageError | RuntimeException ignored) { }
    }
    static void stopViewer() {
        if (loaded) try { nativeViewerStop(); } catch (LinkageError | RuntimeException ignored) { }
    }
    static int viewerPort() {
        if (loaded) try { return nativeViewerPort(); } catch (LinkageError | RuntimeException ignored) { }
        return 0;
    }
    private static native boolean nativeRouteSupported();
    private static native String nativeRouteRun(byte[] offer, String hostname);
    private static native int nativeRouteKind();
    private static native boolean nativeViewerPrepare();
    private static native void nativeViewerStop();
    private static native void nativeViewerAbandonPrepared();
    private static native int nativeViewerPort();
}
