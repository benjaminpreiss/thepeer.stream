package dev.webp2p.controllerprobe;

import android.app.Activity;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.graphics.Color;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

/** Camera consent and browser navigation live in the foreground Activity only.
 * The connection and one-shot launch state survive Activity recreation in ViewerService. */
public final class MainActivity extends Activity {
    private static final int CAMERA_PERMISSION = 41;
    private final Handler handler = new Handler(Looper.getMainLooper());
    private TextView status, details, notice;
    private Button connect, open, stop;
    private ViewerQrScanner scanner;
    private boolean resumed, permissionPending;
    private final Runnable refresh = new Runnable() {
        @Override public void run() {
            if (!resumed) return;
            render();
            // Never launch a browser from a background service or on every resume.
            if (ViewerService.takeBrowserLaunch()) openApp();
            handler.postDelayed(this, 250);
        }
    };

    @Override public void onCreate(Bundle saved) {
        super.onCreate(saved);
        LinearLayout root = column();
        root.setFitsSystemWindows(true);
        root.setBackgroundColor(Color.rgb(247, 249, 252));
        int pad = dp(24);
        root.setPadding(pad, pad, pad, dp(16));
        root.setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS);
        if (android.os.Build.VERSION.SDK_INT >= 29)
            root.setImportantForContentCapture(View.IMPORTANT_FOR_CONTENT_CAPTURE_NO_EXCLUDE_DESCENDANTS);
        ScrollView scroll = new ScrollView(this);
        LinearLayout content = column();
        TextView title = text("thepeer", 28);
        title.setPadding(0, dp(16), 0, dp(24));
        content.addView(title);
        status = text("", 22); content.addView(status);
        notice = text("Scan only a QR from a computer you trust. Scanning connects and opens its app in your browser.", 16);
        notice.setPadding(0, dp(16), 0, dp(16)); content.addView(notice);
        connect = new Button(this); connect.setText("Connect");
        connect.setOnClickListener(view -> scan()); content.addView(connect);
        details = text("", 16); details.setTextIsSelectable(true);
        details.setPadding(0, dp(16), 0, dp(16)); content.addView(details);
        open = new Button(this); open.setText("Open app");
        open.setOnClickListener(view -> openApp()); content.addView(open);
        scroll.addView(content);
        root.addView(scroll, new LinearLayout.LayoutParams(-1, 0, 1));
        // Sibling of the scrolling content: always reachable, even with long details.
        stop = new Button(this); stop.setText("Stop");
        stop.setOnClickListener(view -> {
            notice.setText("");
            ViewerService.stop(getApplicationContext());
            render();
        });
        root.addView(stop, new LinearLayout.LayoutParams(-1, -2));
        setContentView(root);
        discardLaunchData();
        render();
    }

    private int dp(int value) { return Math.round(value * getResources().getDisplayMetrics().density); }
    private LinearLayout column() {
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        layout.setSaveEnabled(false);
        return layout;
    }
    private TextView text(String value, int size) {
        TextView view = new TextView(this);
        view.setText(value); view.setTextSize(size); view.setTextColor(Color.rgb(25, 38, 55));
        view.setSaveEnabled(false);
        return view;
    }
    private void render() {
        ConnectionState.Snapshot state = ViewerService.snapshot();
        status.setText(state.message);
        details.setText(state.details);
        details.setVisibility(state.ready ? View.VISIBLE : View.GONE);
        connect.setVisibility(state.active ? View.GONE : View.VISIBLE);
        connect.setEnabled(resumed && !permissionPending && scanner == null);
        open.setVisibility(state.ready ? View.VISIBLE : View.GONE);
        stop.setVisibility(state.active ? View.VISIBLE : View.GONE);
        stop.setEnabled(!state.stopping);
    }
    private void openApp() {
        if (!resumed) return;
        ConnectionState.Snapshot state = ViewerService.snapshot();
        if (!state.ready) return;
        ViewerService.takeBrowserLaunch(); // Manual Open also consumes any pending automatic attempt.
        Intent browser = new Intent(Intent.ACTION_VIEW, Uri.parse(state.url));
        browser.addCategory(Intent.CATEGORY_BROWSABLE);
        try { startActivity(browser); }
        catch (RuntimeException error) {
            notice.setText("Could not open a browser. Use Open app to retry or the displayed address; do not replace the .localhost hostname.");
        }
    }
    private void scan() {
        if (!resumed || permissionPending || scanner != null || ViewerService.snapshot().active) return;
        if (checkSelfPermission(android.Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED) {
            permissionPending = true;
            render();
            try { requestPermissions(new String[] { android.Manifest.permission.CAMERA }, CAMERA_PERMISSION); }
            catch (RuntimeException error) {
                permissionPending = false;
                notice.setText("Could not request camera permission. Tap Connect to retry.");
            }
            return;
        }
        notice.setText("");
        try {
            scanner = new ViewerQrScanner(this, launch -> {
                if (!resumed || isFinishing() || isDestroyed() || ViewerService.snapshot().active) { launch.close(); return; }
                if (!NativeSaorsaProbe.routingSupported()) {
                    launch.close();
                    notice.setText("This native build cannot use wpa2 offers. Rebuild with native-build.sh and fully reinstall the app. No legacy fallback was attempted.");
                    return;
                }
                ViewerService.start(getApplicationContext(), launch);
                render();
            }, notice::setText, () -> scanner = null);
            scanner.show();
            render();
        } catch (RuntimeException error) {
            if (scanner != null) scanner.cancel();
            scanner = null;
            notice.setText("Camera scanner unavailable. Tap Connect to retry.");
        }
    }
    @Override public void onRequestPermissionsResult(int code, String[] permissions, int[] results) {
        super.onRequestPermissionsResult(code, permissions, results);
        if (code != CAMERA_PERMISSION) return;
        permissionPending = false;
        // A deferred permission result must not open the camera after leaving/recreation.
        boolean granted = results.length == 1 && results[0] == PackageManager.PERMISSION_GRANTED;
        notice.setText(granted ? "Camera permission granted. Tap Connect to scan."
                : "Camera permission is needed to scan. Tap Connect to try again.");
    }
    private void discardLaunchData() {
        // Only explicit camera scans connect. Never accept grants or browser URLs via Intents.
        Intent incoming = getIntent();
        if (incoming != null) {
            incoming.setData(null); incoming.replaceExtras((Bundle) null); incoming.setClipData(null);
        }
        setIntent(new Intent(this, MainActivity.class));
    }
    @Override protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        if (scanner != null) scanner.cancel();
        setIntent(intent); discardLaunchData();
    }
    @Override public void onResume() {
        super.onResume(); resumed = true;
        handler.removeCallbacks(refresh); handler.post(refresh);
    }
    @Override public void onPause() {
        resumed = false;
        if (scanner != null) scanner.cancel();
        handler.removeCallbacks(refresh);
        super.onPause();
    }
    @Override public void onDestroy() {
        if (scanner != null) scanner.cancel();
        handler.removeCallbacksAndMessages(null);
        super.onDestroy();
    }
}
