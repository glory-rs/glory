package rs.glory.counter;

import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.os.Handler;
import android.os.Looper;
import java.lang.ref.WeakReference;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;

// App-owned Android host. No clipboard listener, URI coercion, native exports,
// permission declaration or strong Activity reference is introduced.
public final class GloryClipboard {
    public static final class Coordinator {
        private final WeakReference<Activity> activity;
        private final ClipboardLedger ledger;
        private volatile boolean resumed;
        public Coordinator(Activity activity) {
            this.activity=new WeakReference<>(activity);
            Handler main=new Handler(Looper.getMainLooper());
            ledger=new ClipboardLedger(main::post,this::execute);
        }
        public Object open() { return ledger.open(); }
        public void resumed(boolean value) { resumed=value; }
        public void close() { ledger.close(); activity.clear(); }
        private ClipboardLedger.Reply execute(byte[] write,int limit) {
            Activity current=activity.get();
            if(!resumed || current==null || current.isFinishing() || current.isDestroyed() || !current.hasWindowFocus()) return ClipboardLedger.Reply.code(ClipboardLedger.UNAVAILABLE);
            try {
                ClipboardManager clipboard=current.getSystemService(ClipboardManager.class);
                if(clipboard==null) return ClipboardLedger.Reply.code(ClipboardLedger.UNAVAILABLE);
                if(write!=null) {
                    String text=StandardCharsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT).onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(write)).toString();
                    clipboard.setPrimaryClip(ClipData.newPlainText("Glory",text));
                    return ClipboardLedger.Reply.code(ClipboardLedger.WRITTEN);
                }
                ClipData data=clipboard.getPrimaryClip();
                if(data==null || data.getItemCount()==0) return ClipboardLedger.Reply.code(ClipboardLedger.UNAVAILABLE);
                CharSequence text=data.getItemAt(0).getText();
                if(text==null) return ClipboardLedger.Reply.code(ClipboardLedger.UNAVAILABLE);
                return new ClipboardLedger.Reply(ClipboardLedger.TEXT,ClipboardLedger.encode(text,limit));
            } catch(ClipboardLedger.Budget error) { return ClipboardLedger.Reply.code(ClipboardLedger.TOO_LONG); }
            catch(SecurityException error) { return ClipboardLedger.Reply.code(ClipboardLedger.DENIED); }
            catch(CharacterCodingException | RuntimeException error) { return ClipboardLedger.Reply.code(ClipboardLedger.FAILED); }
        }
    }
}
