package dev.webp2p.controllerprobe;

import com.google.zxing.DecodeHintType;
import com.google.zxing.LuminanceSource;
import com.google.zxing.MultiFormatReader;
import com.google.zxing.ReaderException;
import com.google.zxing.Result;
import com.journeyapps.barcodescanner.Decoder;
import com.journeyapps.barcodescanner.DecoderFactory;
import java.util.Map;

/** Adapts the bounded QR-only decoder to JourneyApps 4.3.0's camera worker. */
final class EmbeddedQrDecoderFactory implements DecoderFactory {
    @Override public Decoder createDecoder(Map<DecodeHintType, ?> ignored) {
        return new Decoder(new MultiFormatReader()) {
            @Override public Result decode(LuminanceSource source) {
                try { return RelayQrDecoder.decode(source); }
                catch (ReaderException | RuntimeException error) { return null; }
            }
        };
    }
}
