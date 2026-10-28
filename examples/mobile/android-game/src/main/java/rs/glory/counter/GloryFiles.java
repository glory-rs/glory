package rs.glory.counter;

import android.app.Activity;
import android.content.ClipData;
import android.content.ContentResolver;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Handler;
import android.os.Looper;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;
import android.system.ErrnoException;
import android.system.Os;
import android.system.OsConstants;
import android.webkit.MimeTypeMap;
import java.io.IOException;
import java.lang.ref.WeakReference;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

// All selection and reads are explicit app-owned content URI grants. No names
// become paths, no persistent grants/storage permission, native export or unsafe.
public final class GloryFiles {
    // Reflection keeps the opt-in trace class out of ordinary/SDK-injection APKs.
    private static void trace(int index) {
        if(BuildConfig.GLORY_FILES_UI_PROBE) {
            try { Class.forName("rs.glory.counter.GlorySystemFilesTrace").getMethod("mark",int.class).invoke(null,index); }
            catch(ReflectiveOperationException error) { throw new IllegalStateException("missing system-files trace",error); }
        }
    }
    private static void launched(Intent intent) {
        if(BuildConfig.GLORY_FILES_UI_PROBE) {
            android.util.Log.i("GlorySystemFiles","ACTION_OPEN_DOCUMENT launched explicit="+(intent.getComponent()!=null));
            trace(0);
        }
    }
    private static FilesLedger shared;
    private static synchronized FilesLedger shared() { if(shared==null) shared=new FilesLedger(); return shared; }
    public static final class Coordinator {
        private final WeakReference<Activity> activity;
        private final ContentResolver resolver;
        private final FilesLedger ledger;
        private final FilesLedger.Session session;
        private volatile boolean resumed;
        public Coordinator(Activity activity,int restoredCode) {
            this.activity=new WeakReference<>(activity);
            resolver=activity.getApplicationContext().getContentResolver();
            ledger=shared(); ledger.restoreCode(restoredCode);
            Handler main=new Handler(Looper.getMainLooper());
            session=ledger.session(main::post,this::launch);
        }
        public Object open() { return session.open(); }
        public void resumed(boolean value) { resumed=value; }
        public boolean ready() { Activity current=activity.get(); return resumed && current!=null && current.hasWindowFocus(); }
        public int nextCode() { return ledger.nextCode(); }
        public void close() { session.close(); activity.clear(); }
        @SuppressWarnings("deprecation")
        private boolean launch(int code,FilesLedger.Config config) {
            Activity current=activity.get();
            if(!resumed || current==null || current.isDestroyed() || current.isFinishing() || !current.hasWindowFocus()) return false;
            Intent intent=new Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
            ArrayList<String> mime=new ArrayList<>(); boolean broad=config.filters.isEmpty();
            for(String filter:config.filters) {
                String hint=filter.startsWith(".") ? MimeTypeMap.getSingleton().getMimeTypeFromExtension(filter.substring(1)) : filter;
                if(hint==null) broad=true; else if(!mime.contains(hint)) mime.add(hint);
            }
            intent.setType("*/*");
            if(!broad && !mime.isEmpty()) intent.putExtra(Intent.EXTRA_MIME_TYPES,mime.toArray(new String[0]));
            intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE,config.multiple);
            // The acceptance-only APK uses an owned selector Activity. It still
            // receives a real Android activity result and reads a real provider;
            // it does not establish system DocumentsUI/user-input acceptance.
            if(BuildConfig.GLORY_FILES_PROBE) intent.setClassName(current,"rs.glory.counter.GloryFilesSelectionProbe");
            current.startActivityForResult(intent,code); launched(intent); return true;
        }
        public boolean result(int code,int result,Intent data) {
            if(!ledger.owns(code)) return false;
            trace(result==Activity.RESULT_OK ? 1 : 2);
            FilesLedger.Config config=ledger.wanted(code);
            if(config==null) trace(7);
            if(config==null || result!=Activity.RESULT_OK) { ledger.result(code,null); return true; }
            if(data==null || (data.getFlags()&Intent.FLAG_GRANT_READ_URI_PERMISSION)==0) { ledger.failResult(code,FilesLedger.UNAVAILABLE); return true; }
            try {
                ArrayList<Uri> uris=new ArrayList<>(); ClipData clip=data.getClipData();
                if(clip!=null) {
                    if(clip.getItemCount()==0 || clip.getItemCount()>config.count) throw new FilesLedger.Refusal(FilesLedger.COUNT);
                    for(int i=0;i<clip.getItemCount();i++) uris.add(clip.getItemAt(i).getUri());
                } else { uris.add(data.getData()); }
                int bytes=0;
                for(Uri uri:uris) {
                    if(uri==null || !"content".equals(uri.getScheme()) || uri.getAuthority()==null || uri.getFragment()!=null) throw new FilesLedger.Refusal(FilesLedger.TYPE);
                    if("com.android.providers.downloads.documents".equals(uri.getAuthority()) || "com.android.externalstorage.documents".equals(uri.getAuthority()) || "com.android.providers.media.documents".equals(uri.getAuthority())) trace(5);
                    String text=uri.toString();
                    if(text.length()>4096) throw new FilesLedger.Refusal(FilesLedger.METADATA);
                    int length=text.getBytes(StandardCharsets.UTF_8).length; bytes+=length;
                    if(length>4096 || bytes>128*1024) throw new FilesLedger.Refusal(FilesLedger.METADATA);
                }
                List<FilesLedger.Opener> openers=new ArrayList<>();
                for(Uri uri:uris) openers.add(() -> openUri(resolver,uri));
                ledger.result(code,openers);
            } catch(FilesLedger.Refusal error) { ledger.failResult(code,error.code); }
            catch(RuntimeException error) { ledger.failResult(code,FilesLedger.FAILED); }
            return true;
        }
    }
    private static FilesLedger.Offered openUri(ContentResolver resolver,Uri uri) throws IOException {
        String name;
        try(Cursor cursor=resolver.query(uri,new String[]{OpenableColumns.DISPLAY_NAME},null,null,null)) {
            if(cursor==null || !cursor.moveToFirst()) throw new FilesLedger.Refusal(FilesLedger.METADATA);
            name=cursor.getString(cursor.getColumnIndexOrThrow(OpenableColumns.DISPLAY_NAME));
            if(name==null || name.length()>1024) throw new FilesLedger.Refusal(FilesLedger.METADATA);
        }
        String type=resolver.getType(uri); if(type==null) type="";
        if(type.length()>127) throw new FilesLedger.Refusal(FilesLedger.METADATA);
        ParcelFileDescriptor descriptor=resolver.openFileDescriptor(uri,"r");
        if(descriptor==null) throw new FilesLedger.Refusal(FilesLedger.UNAVAILABLE);
        trace(3);
        try {
            if((Os.fstat(descriptor.getFileDescriptor()).st_mode&OsConstants.S_IFMT)!=OsConstants.S_IFREG) throw new FilesLedger.Refusal(FilesLedger.BYTES);
            long size=descriptor.getStatSize();
            if(size<0 || size>FilesLedger.MAX_BYTES) throw new FilesLedger.Refusal(FilesLedger.BYTES);
            ParcelFileDescriptor.AutoCloseInputStream stream=new ParcelFileDescriptor.AutoCloseInputStream(descriptor);
            FileChannel channel=stream.getChannel(); channel.position(); // Refuse non-seekable provider streams.
            FilesLedger.Source source=new FilesLedger.Source() {
                @Override public byte[] read(long offset,int length) throws IOException {
                    byte[] bytes=new byte[length]; ByteBuffer output=ByteBuffer.wrap(bytes);
                    while(output.hasRemaining()) { int read=channel.read(output,offset+output.position()); if(read<=0) throw new IOException("short provider read"); }
                    trace(6); return bytes;
                }
                @Override public void close() throws IOException { try { stream.close(); } finally { descriptor.close(); trace(4); } }
            };
            return new FilesLedger.Offered(name,type,size,source);
        } catch(IOException | RuntimeException | ErrnoException error) {
            try { descriptor.close(); trace(4); } catch(IOException ignored) {}
            if(error instanceof IOException) throw (IOException)error;
            throw new IOException("provider descriptor unavailable",error);
        }
    }
}
