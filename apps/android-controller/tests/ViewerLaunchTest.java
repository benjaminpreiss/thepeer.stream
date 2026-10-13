package dev.webp2p.controllerprobe;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Base64;

/** Public synthetic data only. Full semantic validation belongs to the native decoder. */
public final class ViewerLaunchTest {
    static String offer() {
        byte[] p = new byte[73]; p[0] = 2;
        Arrays.fill(p, 2, 34, (byte) 0x11); Arrays.fill(p, 34, 66, (byte) 0x22);
        p[66] = 4; p[67] = 127; p[70] = 1; p[71] = 0x30; p[72] = 0x39;
        return "wpa2:" + Base64.getUrlEncoder().withoutPadding().encodeToString(p);
    }
    static ViewerLaunch launch() { return ViewerLaunch.parse(offer()); }
    static void check(boolean ok) { if (!ok) throw new AssertionError("Launch assertion; contents omitted"); }
    static void closed(ViewerLaunch value) {
        try { value.takeOffer(); throw new AssertionError("Consumed invitation survived"); }
        catch (IllegalStateException expected) { }
    }
    public static void main(String[] args) {
        String good = offer();
        ViewerLaunch launch = launch();
        check(launch.toString().equals("ViewerLaunch[redacted]"));
        byte[] transferred = launch.takeOffer();
        check(Arrays.equals(transferred, good.getBytes(StandardCharsets.US_ASCII)));
        closed(launch); launch.close();
        check(Arrays.equals(transferred, good.getBytes(StandardCharsets.US_ASCII)));
        Arrays.fill(transferred, (byte) 0);
        ViewerLaunch discarded = launch(); discarded.close(); discarded.close(); closed(discarded);
        for (String bad : new String[] {null, "", good.substring(0, 102), good + "=", " " + good,
                good + "#fragment", "wpa2:" + "A".repeat(191), good.replace("wpa2:", "WPA2:"),
                good.replace("wpa2:", "wpa1:"), good.replace("wpa2:", "wpr1:"), good.replace("wpa2:", "wpt1:"),
                "webp2p-controller://viewer/connect?v=1", "intent://viewer/#Intent;end", "https://example.org/",
                "wpa2:é" + good.substring(6), "wpa2:+" + good.substring(6)}) {
            try { ViewerLaunch.parse(bad); throw new AssertionError("Bad envelope accepted"); }
            catch (IllegalArgumentException expected) { check(expected.getCause() == null); }
        }
        ViewerLaunch maximum = ViewerLaunch.parse("wpa2:" + "A".repeat(190));
        maximum.close(); closed(maximum);
        System.out.println("PASS: target-only envelope, opaque single transfer and close; native code validates packet semantics.");
    }
}
