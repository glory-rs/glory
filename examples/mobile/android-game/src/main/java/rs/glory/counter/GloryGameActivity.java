package rs.glory.counter;

import com.google.androidgamesdk.GameActivity;
import android.app.Application;
import android.os.Bundle;

// The visible UI and its state remain in Rust. GameActivity supplies the Android
// Surface and the system InputConnection through its matching GameTextInput.
public final class GloryGameActivity extends GameActivity {
    static { System.loadLibrary("glory_mobile_counter"); }
    private static volatile Application applicationContext;

    @Override public void onCreate(Bundle state) {
        // Application lifetime survives Activity recreation. Never store an
        // Activity in the TLS verifier's process-wide application context.
        applicationContext = getApplication();
        super.onCreate(state);
    }

    public static Object gloryApplicationContext() { return applicationContext; }
}
