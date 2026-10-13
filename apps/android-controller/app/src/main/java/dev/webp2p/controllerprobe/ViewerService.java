package dev.webp2p.controllerprobe;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.os.Build;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;

/** Foreground native owner. No session timer, persisted grant, automatic restart or reconnect. */
public final class ViewerService extends Service {
    private static final String CHANNEL = "native-target-viewer";
    private static final String GENERATION = "connection-generation"; // Not an authorization capability.
    private static final ConnectionState STATE = new ConnectionState();
    private static final ConnectionHandoff HANDOFF = new ConnectionHandoff();
    private final Handler main = new Handler(Looper.getMainLooper());
    private Thread worker;
    private long owner;
    private boolean destroyed, finished;
    private String result = "Disconnected. Connect with a fresh helper QR.";

    static ConnectionState.Snapshot snapshot() { return STATE.snapshot(NativeSaorsaProbe.viewerPort(), NativeSaorsaProbe.routeKind()); }
    static boolean takeBrowserLaunch() { return STATE.takeBrowserLaunch(NativeSaorsaProbe.viewerPort()); }

    static synchronized void start(Context context, ViewerLaunch launch) {
        String hostname = "thepeer-" + java.util.UUID.randomUUID().toString().replace("-", "") + ".localhost";
        if (!NativeSaorsaProbe.routingSupported()) { launch.close(); return; }
        if (!STATE.begin(launch, hostname)) { launch.close(); return; }
        long generation = HANDOFF.offer(launch, hostname);
        if (generation == 0) { STATE.finish("Connection unavailable."); return; }
        try {
            context.startForegroundService(new Intent(context, ViewerService.class).putExtra(GENERATION, generation));
        } catch (RuntimeException error) {
            HANDOFF.cancelPending();
            STATE.finish("Could not start the connection. Try a fresh helper QR.");
        }
    }

    static synchronized void stop(Context context) {
        if (HANDOFF.cancelPending()) {
            // No native owner exists yet. Do not wait for a service that may never start.
            STATE.finish("Stopped. Connect with a fresh helper QR.");
        } else {
            STATE.stop();
            NativeSaorsaProbe.stopViewer();
        }
        // Claimed ownership remains until BOTH the worker and service have ended.
        context.stopService(new Intent(context, ViewerService.class));
    }

    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        long generation = intent == null ? 0 : intent.getLongExtra(GENERATION, 0);
        if (intent != null && "STOP".equals(intent.getAction())) {
            if (generation == owner && HANDOFF.owns(owner)) {
                STATE.stop(); NativeSaorsaProbe.stopViewer(); stopSelf();
            } else if (owner == 0) stopSelf(startId);
            return START_NOT_STICKY;
        }
        if (worker != null || owner != 0) return START_NOT_STICKY;
        final ConnectionHandoff.Arguments arguments;
        synchronized (ViewerService.class) {
            arguments = HANDOFF.claim(generation);
            if (arguments != null) owner = arguments.generation;
        }
        if (arguments == null) { stopSelf(startId); return START_NOT_STICKY; }
        boolean prepared = false;
        try {
            NotificationManager manager = getSystemService(NotificationManager.class);
            manager.createNotificationChannel(new NotificationChannel(CHANNEL, "App connection", NotificationManager.IMPORTANCE_LOW));
            PendingIntent open = PendingIntent.getActivity(this, 20, new Intent(this, MainActivity.class), PendingIntent.FLAG_IMMUTABLE);
            PendingIntent stop = PendingIntent.getService(this, 21,
                    new Intent(this, ViewerService.class).setAction("STOP").putExtra(GENERATION, owner),
                    PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
            Notification notification = new Notification.Builder(this, CHANNEL)
                    .setSmallIcon(android.R.drawable.ic_menu_info_details)
                    .setContentTitle(getString(R.string.app_name))
                    .setContentText("Connection active or connecting · tap for details")
                    .setContentIntent(open).setOngoing(true)
                    .addAction(new Notification.Action.Builder(null, "Stop", stop).build()).build();
            if (Build.VERSION.SDK_INT >= 34) startForeground(2, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE);
            else startForeground(2, notification);
            prepared = NativeSaorsaProbe.prepareViewer();
            if (!prepared) {
                arguments.close();
                result = "Connection unavailable or busy. Rebuild the native library and restart the app.";
                stopSelf();
                return START_NOT_STICKY;
            }
            if (STATE.snapshot(0).stopping) NativeSaorsaProbe.stopViewer();
            worker = new Thread(() -> run(arguments), "native-application");
            worker.start();
        } catch (RuntimeException | Error error) {
            arguments.close();
            if (prepared) NativeSaorsaProbe.abandonPreparedViewer();
            worker = null;
            finished = true;
            result = "Could not start the foreground connection (details omitted).";
            stopSelf();
        }
        return START_NOT_STICKY;
    }

    private void run(ConnectionHandoff.Arguments arguments) {
        ViewerLaunch launch = arguments.launch;
        String outcome;
        try {
            outcome = NativeSaorsaProbe.runRouting(launch.takeOffer(), arguments.hostname);
        } catch (RuntimeException | Error error) {
            outcome = "Connection failed (details omitted). Use a fresh helper QR.";
        } finally {
            arguments.close();
            // Also covers an exception before entering JNI; never releases a running reservation.
            NativeSaorsaProbe.abandonPreparedViewer();
        }
        final String report = outcome;
        main.post(() -> {
            result = report;
            finished = true;
            stopSelf();
            if (destroyed) releaseOwner();
        });
    }

    private void releaseOwner() {
        synchronized (ViewerService.class) {
            if (HANDOFF.release(owner)) STATE.finish(result);
        }
    }

    @Override public void onDestroy() {
        destroyed = true;
        if (HANDOFF.owns(owner)) {
            STATE.stop();
            NativeSaorsaProbe.stopViewer();
        }
        stopForeground(STOP_FOREGROUND_REMOVE);
        super.onDestroy();
        if (worker == null || finished) releaseOwner();
    }
    @Override public IBinder onBind(Intent intent) { return null; }
}
