package rs.glory.counter;

import android.app.Activity;
import android.content.ClipData;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import java.util.concurrent.atomic.AtomicInteger;

// Compiled/registered only in files-probe. This owned source is deliberately
// separate from system DocumentsUI and never opens ambient user documents.
public final class GloryFilesSelectionProbe extends Activity {
    private static final AtomicInteger next=new AtomicInteger();
    public static void configure(int mode) { next.set(mode); }
    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        int mode=next.getAndSet(0);
        if(mode==1) { setResult(RESULT_CANCELED); finish(); return; }
        Uri png=Uri.parse("content://rs.glory.counter.files-probe/badge");
        Intent result=new Intent().addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
        if(mode==2) {
            ClipData clip=ClipData.newRawUri("Owned excessive count",png);
            for(int i=1;i<33;i++) clip.addItem(new ClipData.Item(png));
            result.setClipData(clip);
        } else if(mode==3) result.setData(Uri.parse("file:///owned-source-not-allowed.png"));
        else if(mode==4) result.setData(png).setFlags(0);
        else if(getIntent().getBooleanExtra(Intent.EXTRA_ALLOW_MULTIPLE,false)) {
            ClipData clip=ClipData.newRawUri("Owned files",Uri.parse("content://rs.glory.counter.files-probe/text"));
            clip.addItem(new ClipData.Item(png)); result.setClipData(clip);
        } else result.setData(png);
        setResult(RESULT_OK,result); finish();
    }
}
