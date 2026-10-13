package dev.webp2p.controllerprobe;

import com.google.zxing.BarcodeFormat;
import com.google.zxing.RGBLuminanceSource;
import com.google.zxing.qrcode.QRCodeWriter;
import com.google.zxing.common.BitMatrix;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;

/** Synthetic QR rotation checks. The JNI runner independently uses actual helper QR pixels. */
public final class ViewerScanTest {
    private static void check(boolean ok) { if (!ok) throw new AssertionError("Scan assertion; contents omitted"); }
    public static void main(String[] args) throws Exception {
        String good = ViewerLaunchTest.offer();
        BitMatrix qr = new QRCodeWriter().encode(good, BarcodeFormat.QR_CODE, 320, 320);
        for (int rotation = 0; rotation < 4; rotation++) {
            int[] pixels = new int[320 * 320];
            for (int y = 0; y < 320; y++) for (int x = 0; x < 320; x++) {
                int xx = x, yy = y;
                for (int turn = 0; turn < rotation; turn++) { int old = xx; xx = yy; yy = 319 - old; }
                pixels[y * 320 + x] = qr.get(xx, yy) ? 0xff000000 : 0xffffffff;
            }
            String decoded = RelayQrDecoder.decode(new RGBLuminanceSource(320, 320, pixels)).getText();
            check(decoded.equals(good));
            ViewerScan scan = new ViewerScan(100);
            try (ViewerLaunch launch = scan.accept(decoded, 101)) {
                byte[] bytes = launch.takeOffer();
                check(Arrays.equals(bytes, good.getBytes(StandardCharsets.US_ASCII)));
                Arrays.fill(bytes, (byte) 0);
            }
            check(scan.accept(decoded, 102) == null);
        }
        ViewerScan cancelled = new ViewerScan(100); cancelled.cancel();
        check(cancelled.accept(good, 101) == null);
        ViewerScan expired = new ViewerScan(100);
        check(expired.accept(good, 100 + ViewerScan.LIFETIME_MS) == null);
        check(expired.accept(good, 101) == null);
        check(new ViewerScan(100).accept(good, 99) == null);
        try (ViewerLaunch last = new ViewerScan(100).accept(good, 99 + ViewerScan.LIFETIME_MS)) { check(last != null); }
        for (String prefix : new String[] {"wpr1:", "wpa1:", "wpt1:", "https:"}) {
            ViewerScan scan = new ViewerScan(100);
            try { scan.accept(prefix + good.substring(5), 101); throw new AssertionError("Legacy scan accepted"); }
            catch (IllegalArgumentException expected) { }
            check(scan.accept(good, 102) == null);
        }
        System.out.println("PASS: rotated QR decode, target-only handoff, cancel/expiry/duplicates. Not Android camera evidence.");
    }
}
