package rs.glory.counter;

import com.google.androidgamesdk.GameActivity;

// The visible UI and its state remain in Rust. GameActivity supplies the Android
// Surface and the system InputConnection through its matching GameTextInput.
public final class GloryGameActivity extends GameActivity {
    static { System.loadLibrary("glory_mobile_counter"); }
}
