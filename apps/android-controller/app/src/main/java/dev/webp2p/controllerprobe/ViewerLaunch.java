package dev.webp2p.controllerprobe;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;

/** Opaque, single-transfer in-memory invitation. Native code owns full format validation.
 * No networking, legacy translation, grant extraction, intents or persistence. */
final class ViewerLaunch implements AutoCloseable {
    private byte[] offer;
    private ViewerLaunch(byte[] offer) { this.offer = offer; }

    static ViewerLaunch parse(String raw) {
        // Bound the scanner handoff before copying. The native decoder additionally
        // checks canonical base64, version/flags, address structure and eligibility
        // before creating an endpoint; accepting this envelope is not authorization.
        if (raw == null || raw.length() < 103 || raw.length() > 195 || !raw.startsWith("wpa2:")) throw invalid();
        for (int i = 5; i < raw.length(); i++) {
            char c = raw.charAt(i);
            if (!((c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z')
                    || (c >= '0' && c <= '9') || c == '-' || c == '_')) throw invalid();
        }
        return new ViewerLaunch(raw.getBytes(StandardCharsets.US_ASCII));
    }
    synchronized byte[] takeOffer() {
        if (offer == null) throw new IllegalStateException("Invitation already consumed or closed.");
        byte[] transferred = offer;
        offer = null;
        return transferred;
    }
    @Override public synchronized void close() {
        if (offer != null) Arrays.fill(offer, (byte) 0);
        offer = null;
    }
    private static IllegalArgumentException invalid() { return new IllegalArgumentException("Invalid application QR."); }
    @Override public String toString() { return "ViewerLaunch[redacted]"; }
}
