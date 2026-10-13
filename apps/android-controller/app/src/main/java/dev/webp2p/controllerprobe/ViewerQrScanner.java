package dev.webp2p.controllerprobe;

import android.app.Activity;
import android.app.Dialog;
import android.graphics.Color;
import android.os.Handler;
import android.os.Looper;
import android.os.SystemClock;
import android.view.View;
import android.view.Window;
import android.view.WindowManager;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;

import com.google.zxing.ResultPoint;
import com.journeyapps.barcodescanner.BarcodeCallback;
import com.journeyapps.barcodescanner.BarcodeResult;
import com.journeyapps.barcodescanner.BarcodeView;
import com.journeyapps.barcodescanner.CameraPreview;
import com.journeyapps.barcodescanner.camera.FitCenterStrategy;

import java.util.List;
import java.util.function.Consumer;

/** Local, embedded preview/decoder: no CaptureActivity, result Intent, image save or browser. */
final class ViewerQrScanner {
    private final Dialog dialog;
    private final BarcodeView camera;
    private final ViewerScan scan = new ViewerScan(SystemClock.elapsedRealtime());
    private final Handler handler = new Handler(Looper.getMainLooper());
    private final Consumer<ViewerLaunch> accepted;
    private final Consumer<String> status;
    private final Runnable closed;
    private boolean active = true;
    private final Runnable expire = () -> finish(null, "Scan expired. Camera closed; nothing changed.");

    ViewerQrScanner(Activity activity, Consumer<ViewerLaunch> accepted,
                    Consumer<String> status, Runnable closed) {
        this.accepted = accepted; this.status = status; this.closed = closed;
        dialog = new Dialog(activity);
        dialog.requestWindowFeature(Window.FEATURE_NO_TITLE);
        LinearLayout layout = new LinearLayout(activity);
        layout.setOrientation(LinearLayout.VERTICAL);
        layout.setPadding(16, 16, 16, 16);
        layout.setBackgroundColor(Color.BLACK);
        layout.setSaveEnabled(false);
        layout.setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS);
        if (android.os.Build.VERSION.SDK_INT >= 29)
            layout.setImportantForContentCapture(View.IMPORTANT_FOR_CONTENT_CAPTURE_NO_EXCLUDE_DESCENDANTS);
        TextView instructions = new TextView(activity);
        instructions.setTextColor(Color.WHITE);
        instructions.setText("Scan your trusted computer's application QR. Keep the entire QR visible.\nA valid application QR connects and opens your browser. Local decoding only; camera closes after 60 seconds.");
        layout.addView(instructions);
        camera = new BarcodeView(activity);
        camera.setUseTextureView(true); // Avoid SurfaceView overlap in a dialog.
        camera.setPreviewScalingStrategy(new FitCenterStrategy());
        camera.setMarginFraction(0.05);
        camera.setSaveEnabled(false);
        camera.setDecoderFactory(new EmbeddedQrDecoderFactory());
        layout.addView(camera, new LinearLayout.LayoutParams(-1, 0, 1));
        Button cancel = new Button(activity);
        cancel.setText("Cancel scan");
        cancel.setOnClickListener(view -> cancel());
        layout.addView(cancel);
        dialog.setContentView(layout);
        dialog.setCanceledOnTouchOutside(false);
        dialog.setOnDismissListener(ignored -> cancel()); // Includes Back and external dismissal.
        Window window = dialog.getWindow();
        if (window != null) {
            window.addFlags(WindowManager.LayoutParams.FLAG_SECURE);
            window.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_HIDDEN);
        }
        camera.addStateListener(new CameraPreview.StateListener() {
            public void previewSized() { }
            public void previewStarted() { }
            public void previewStopped() { }
            public void cameraClosed() { }
            public void cameraError(Exception error) {
                finish(null, "Camera unavailable. No settings changed.");
            }
        });
    }

    /** Caller must already hold CAMERA permission. Never resume automatically after leaving. */
    void show() {
        if (!active) return;
        try {
            dialog.show();
            Window window = dialog.getWindow();
            if (window != null) window.setLayout(-1, -1);
            handler.postDelayed(expire, ViewerScan.LIFETIME_MS);
            camera.decodeSingle(new BarcodeCallback() {
                public void barcodeResult(BarcodeResult result) {
                    if (!active) return;
                    try {
                        ViewerLaunch launch = scan.accept(result == null ? null : result.getText(), SystemClock.elapsedRealtime());
                        finish(launch, launch == null ? "Scan expired or cancelled; no settings changed." : null);
                    } catch (IllegalArgumentException error) {
                        finish(null, "Invalid viewer QR. Camera closed; no settings changed. Use a fresh helper QR.");
                    }
                }
                public void possibleResultPoints(List<ResultPoint> points) { }
            });
            camera.resume();
        } catch (RuntimeException error) {
            finish(null, "Camera could not start. No settings changed (details omitted).");
        }
    }

    void cancel() { finish(null, "Scan cancelled. Camera closed; no settings changed."); }

    private void finish(ViewerLaunch launch, String message) {
        if (!active) { if (launch != null) launch.close(); return; }
        active = false;
        scan.cancel();
        handler.removeCallbacks(expire);
        // Pause asynchronously releases the camera on the library's serial camera
        // thread; do not block the UI waiting for a hardware close. Queued results
        // cannot apply after cancellation because active/scan have already closed.
        boolean clean = true;
        try { camera.stopDecoding(); } catch (RuntimeException error) { clean = false; }
        try { camera.pause(); } catch (RuntimeException error) { clean = false; }
        try { dialog.dismiss(); } catch (RuntimeException error) { clean = false; }
        closed.run();
        if (!clean) {
            if (launch != null) launch.close();
            status.accept("Scanner cleanup failed; no settings changed (details omitted).");
        }
        else if (launch != null) accepted.accept(launch);
        else if (message != null) status.accept(message);
    }
}
