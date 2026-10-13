package dev.webp2p.controllerprobe;

import java.io.DataInputStream;
import java.io.ByteArrayOutputStream;
import java.net.Socket;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicReference;

/** Actual JNI owner and browser. Offer arrives only over an anonymous pipe. */
public final class RoutingPhoneFixture {
    private static int checks;
    private static void check(boolean value) {
        checks++;
        if (!value) throw new AssertionError("Routing JNI fixture failed; contents omitted");
    }
    public static void main(String[] args) {
        try { run(); }
        catch (Throwable failure) {
            NativeSaorsaProbe.stopViewer();
            System.err.println("Routing JNI fixture failed after check " + checks + "; contents omitted");
            System.exit(1);
        }
    }
    private static void run() throws Exception {
        check(NativeSaorsaProbe.routingSupported());
        String host = "thepeer-" + UUID.randomUUID().toString().replace("-", "") + ".localhost";
        // Exercise real JNI reservation consumption and caller-array clearing.
        byte[] invalid = ("wpa1:" + "A".repeat(98)).getBytes(StandardCharsets.US_ASCII);
        check(NativeSaorsaProbe.prepareViewer());
        check(NativeSaorsaProbe.runRouting(invalid, host).contains("stage=route-offer-format"));
        check(Arrays.equals(invalid, new byte[invalid.length]));
        check(NativeSaorsaProbe.viewerPort() == 0 && NativeSaorsaProbe.routeKind() == 0);
        check(NativeSaorsaProbe.prepareViewer());
        NativeSaorsaProbe.stopViewer();
        check(NativeSaorsaProbe.runRouting(invalid, host).startsWith("Connection stopped."));
        check(NativeSaorsaProbe.viewerPort() == 0);

        DataInputStream input = new DataInputStream(System.in);
        int length = input.readUnsignedShort();
        check(length >= 103 && length <= 195);
        byte[] received = new byte[length];
        input.readFully(received);
        final byte[] offer;
        try (ViewerLaunch launch = new ViewerScan(100).accept(new String(received, StandardCharsets.US_ASCII), 101)) {
            offer = launch.takeOffer();
        } finally { Arrays.fill(received, (byte) 0); }
        check(NativeSaorsaProbe.prepareViewer());
        AtomicReference<String> result = new AtomicReference<>();
        Thread worker = new Thread(() -> result.set(NativeSaorsaProbe.runRouting(offer, host)), "routing-jni-fixture");
        worker.start();
        int port = 0;
        long deadline = System.nanoTime() + 10_000_000_000L;
        while (port == 0 && worker.isAlive() && System.nanoTime() < deadline) {
            port = NativeSaorsaProbe.viewerPort();
            if (port == 0) Thread.sleep(10);
        }
        check(port > 0 && NativeSaorsaProbe.routeKind() == 1);
        // JNI clears the transferred Java capability before the port is published.
        check(Arrays.equals(offer, new byte[offer.length]));
        try (Socket browser = new Socket()) {
            browser.connect(new InetSocketAddress("127.0.0.1", port), 5000);
            browser.setSoTimeout(5000);
            browser.getOutputStream().write(("GET /helper HTTP/1.1\r\nHost: " + host + ":" + port
                    + "\r\nConnection: close\r\n\r\n").getBytes(StandardCharsets.US_ASCII));
            ByteArrayOutputStream bytes = new ByteArrayOutputStream();
            byte[] chunk = new byte[4096];
            int count;
            while ((count = browser.getInputStream().read(chunk)) != -1) {
                check(bytes.size() + count <= 65536);
                bytes.write(chunk, 0, count);
            }
            String response = bytes.toString(StandardCharsets.US_ASCII);
            check(response.startsWith("HTTP/1.1 200") || response.startsWith("HTTP/1.0 200"));
            check(!response.toLowerCase(java.util.Locale.ROOT).contains("content-security-policy:"));
            check(response.contains("from-target"));
        }
        System.out.println("HTTP_OK");
        System.out.flush();
        check(input.read() != -1);
        NativeSaorsaProbe.stopViewer();
        check(NativeSaorsaProbe.viewerPort() == 0 && NativeSaorsaProbe.routeKind() == 0);
        worker.join(20000);
        check(!worker.isAlive());
        check(result.get() != null && result.get().startsWith("Connection stopped."));
        boolean refused = false;
        try (Socket browser = new Socket()) {
            browser.connect(new InetSocketAddress("127.0.0.1", port), 1000);
        } catch (java.io.IOException expected) { refused = true; }
        check(refused);
        check(NativeSaorsaProbe.prepareViewer());
        NativeSaorsaProbe.abandonPreparedViewer();
        check(NativeSaorsaProbe.prepareViewer());
        NativeSaorsaProbe.abandonPreparedViewer();
    }
}
