package rs.glory.counter;

import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.os.Handler;
import android.os.Looper;
import java.lang.ref.WeakReference;
import java.nio.charset.StandardCharsets;
import java.util.UUID;

// Acceptance-only guard, not a clipboard service/history. Original ClipData
// stays in RAM, including all items/types; no URI is opened or coerced to text.
final class GloryClipboardProbe {
    private final WeakReference<Activity> activity;
    private final Handler main=new Handler(Looper.getMainLooper());
    private final String token="glory-owned-android-clipboard-"+UUID.randomUUID();
    private ClipData original;
    private boolean ready,preparing,finishing,closed;
    private long restored;
    GloryClipboardProbe(Activity activity) { this.activity=new WeakReference<>(activity); }
    synchronized byte[] prepare() {
        if(closed) return null;
        if(ready) return token.getBytes(StandardCharsets.UTF_8);
        if(!preparing) { preparing=true; if(!main.post(this::snapshot)) preparing=false; }
        return null;
    }
    private void snapshot() {
        Activity current=activity.get();
        ClipData snapshot=null; boolean success=false;
        try {
            if(current!=null && !current.isDestroyed() && current.hasWindowFocus()) {
                ClipboardManager manager=current.getSystemService(ClipboardManager.class);
                if(manager!=null) { snapshot=manager.getPrimaryClip(); success=true; }
            }
        } catch(RuntimeException error) { success=false; }
        synchronized(this) { preparing=false; if(!closed && success) { original=snapshot; ready=true; } }
    }
    synchronized long finish() {
        if(closed || !ready) return -1;
        if(restored!=0) return restored;
        if(!finishing) { finishing=true; if(!main.post(this::restore)) { finishing=false; return -1; } }
        return 0;
    }
    private void restore() {
        Activity current=activity.get(); ClipData backup;
        synchronized(this) { if(closed) return; backup=original; }
        long result=-1;
        try {
            if(current!=null && !current.isDestroyed() && current.hasWindowFocus()) {
                ClipboardManager manager=current.getSystemService(ClipboardManager.class);
                ClipData value=manager==null?null:manager.getPrimaryClip();
                CharSequence text=value==null || value.getItemCount()!=1?null:value.getItemAt(0).getText();
                boolean owned=text!=null && text.length()>=token.length();
                for(int i=0;owned && i<token.length();i++) owned=text.charAt(i)==token.charAt(i);
                if(owned) {
                    if(backup==null) manager.clearPrimaryClip(); else manager.setPrimaryClip(backup);
                    ClipData check=manager.getPrimaryClip();
                    result=backup==null ? (check==null?1:-1) : (check!=null && check.getItemCount()==backup.getItemCount() && check.getDescription().getMimeTypeCount()==backup.getDescription().getMimeTypeCount()?1:-1);
                } else result=-2; // Preserve an observed external clipboard change.
            }
        } catch(RuntimeException error) { result=-1; }
        synchronized(this) { original=null; restored=result; finishing=false; }
    }
    synchronized void close() { closed=true; original=null; activity.clear(); }
}
