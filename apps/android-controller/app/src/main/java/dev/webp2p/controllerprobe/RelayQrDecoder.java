package dev.webp2p.controllerprobe;

import com.google.zxing.*;
import com.google.zxing.common.HybridBinarizer;
import java.util.Collections;

/** Bounded QR-only decoder for the embedded scanner. No persistence/logs.
 * A single rotation retry handles a
 * reproducible ZXing 3.5.3 orientation-sensitive detection failure on our IPv6 fixture.
 */
final class RelayQrDecoder {
    static final int MAX_PIXELS = 2_097_152;
    private RelayQrDecoder() { }

    static Result decode(LuminanceSource source) throws ReaderException {
        int width = source.getWidth(), height = source.getHeight();
        if (width <= 0 || height <= 0 || width > 4096 || height > 4096 || (long) width * height > MAX_PIXELS)
            throw NotFoundException.getNotFoundInstance();
        try { return once(source); }
        catch (ReaderException first) {
            // No resolution growth, unbounded retries or alternative barcode formats.
            // Camera sources need not implement rotateCounterClockwise().
            byte[] original = source.getMatrix();
            if (original.length < width * height) throw NotFoundException.getNotFoundInstance();
            byte[] rotated = new byte[width * height];
            try {
                for (int y = 0; y < height; y++) for (int x = 0; x < width; x++)
                    rotated[(width - 1 - x) * height + y] = original[y * width + x];
                Result result = once(new Gray(height, width, rotated));
                ResultPoint[] points = result.getResultPoints();
                if (points != null) for (int i = 0; i < points.length; i++) {
                    if (points[i] != null) points[i] = new ResultPoint(width - 1 - points[i].getY(), points[i].getX());
                }
                return result;
            } finally { java.util.Arrays.fill(rotated, (byte) 0); }
        }
    }
    private static Result once(LuminanceSource source) throws ReaderException {
        MultiFormatReader reader = new MultiFormatReader();
        try {
            return reader.decode(new BinaryBitmap(new HybridBinarizer(source)),
                    Collections.singletonMap(DecodeHintType.POSSIBLE_FORMATS,
                            Collections.singletonList(BarcodeFormat.QR_CODE)));
        } finally { reader.reset(); }
    }
    private static final class Gray extends LuminanceSource {
        private final byte[] bytes;
        Gray(int width, int height, byte[] bytes) { super(width, height); this.bytes = bytes; }
        @Override public byte[] getRow(int y, byte[] row) {
            if (y < 0 || y >= getHeight()) throw new IllegalArgumentException("Invalid image row.");
            if (row == null || row.length < getWidth()) row = new byte[getWidth()];
            System.arraycopy(bytes, y * getWidth(), row, 0, getWidth());
            return row;
        }
        @Override public byte[] getMatrix() { return bytes; }
    }
}
