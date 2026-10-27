package rs.glory.counter;

import com.google.androidgamesdk.GameActivity;
import android.app.Application;
import android.os.Bundle;
import java.lang.ref.WeakReference;

// The visible UI and its state remain in Rust. GameActivity supplies the Android
// Surface and the system InputConnection through its matching GameTextInput.
public final class GloryGameActivity extends GameActivity {
    static { System.loadLibrary("glory_mobile_counter"); }
    private static volatile Application applicationContext;
    private static volatile WeakReference<GloryGameActivity> current = new WeakReference<>(null);
    private GloryPermissions.Coordinator permissions;
    private GloryClipboard.Coordinator clipboard;
    private GloryClipboardProbe clipboardProbe;

    @Override public void onCreate(Bundle state) {
        // Application lifetime survives Activity recreation. Never store an
        // Activity in the TLS verifier's process-wide application context.
        applicationContext = getApplication();
        if (BuildConfig.GLORY_PERMISSIONS) permissions = new GloryPermissions.Coordinator(this);
        if (BuildConfig.GLORY_CLIPBOARD) clipboard = new GloryClipboard.Coordinator(this);
        if (BuildConfig.GLORY_CLIPBOARD_PROBE) clipboardProbe = new GloryClipboardProbe(this);
        // Publish the fully constructed optional coordinator to the Rust thread.
        current = new WeakReference<>(this);
        super.onCreate(state);
    }

    public static Object gloryApplicationContext() { return applicationContext; }
    public static Object gloryPermissionHost() {
        GloryGameActivity activity = current.get();
        return activity == null || activity.permissions == null ? null : activity.permissions.open();
    }
    public static Object gloryClipboardHost() {
        GloryGameActivity activity = current.get();
        return activity == null || activity.clipboard == null ? null : activity.clipboard.open();
    }
    public static byte[] gloryClipboardProbePrepare() {
        GloryGameActivity activity = current.get();
        return activity == null || activity.clipboardProbe == null ? null : activity.clipboardProbe.prepare();
    }
    public static long gloryClipboardProbeFinish() {
        GloryGameActivity activity = current.get();
        return activity == null || activity.clipboardProbe == null ? -1 : activity.clipboardProbe.finish();
    }
    @Override public void onResume() {
        super.onResume();
        if (permissions != null) permissions.resumed(true);
        if (clipboard != null) clipboard.resumed(true);
    }
    @Override public void onPause() {
        if (permissions != null) permissions.resumed(false);
        if (clipboard != null) clipboard.resumed(false);
        super.onPause();
    }
    @Override public void onRequestPermissionsResult(int code, String[] names, int[] grants) {
        if (permissions != null && permissions.result(code, names, grants)) return;
        super.onRequestPermissionsResult(code, names, grants);
    }
    @Override public void onDestroy() {
        if (permissions != null) permissions.close();
        if (clipboard != null) clipboard.close();
        if (clipboardProbe != null) clipboardProbe.close();
        if (current.get() == this) current.clear();
        super.onDestroy();
    }
}
