import com.google.zxing.BinaryBitmap;
import com.google.zxing.DecodeHintType;
import com.google.zxing.RGBLuminanceSource;
import com.google.zxing.common.HybridBinarizer;
import com.google.zxing.qrcode.QRCodeReader;
import java.io.DataInputStream;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Map;

/** Independent decoder; live image and payload travel only through anonymous pipes. */
public final class HelperQrDecode {
    public static void main(String[] args) {
        try {
            DataInputStream input = new DataInputStream(System.in);
            int width = input.readInt(), height = input.readInt();
            if (width < 21 || width > 256 || height < 21 || height > 256) throw new Exception();
            byte[] image = input.readNBytes(width * height);
            if (image.length != width * height) throw new Exception();
            int[] pixels = new int[image.length];
            try {
                for (int i = 0; i < pixels.length; i++) pixels[i] = image[i] == 0 ? 0xffffff : 0;
                var bitmap = new BinaryBitmap(new HybridBinarizer(new RGBLuminanceSource(width, height, pixels)));
                var result = new QRCodeReader().decode(bitmap, Map.of(DecodeHintType.PURE_BARCODE, true));
                byte[] offer = result.getText().getBytes(StandardCharsets.US_ASCII);
                try {
                    if (offer.length > 195) throw new Exception();
                    System.out.write(offer);
                    System.out.flush();
                } finally { Arrays.fill(offer, (byte) 0); }
            } finally { Arrays.fill(image, (byte) 0); Arrays.fill(pixels, 0); }
        } catch (Exception ignored) {
            System.err.println("QR decode failed; contents omitted");
            System.exit(1);
        }
    }
}
