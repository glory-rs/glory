package rs.glory.counter;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.Handler;
import android.os.Looper;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;
import java.io.File;
import java.io.FileNotFoundException;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.atomic.AtomicInteger;

// Acceptance-only, non-exported provider. Files are fixed application-owned
// fixtures, never display-name paths or selected user content.
public final class GloryFilesProbeProvider extends ContentProvider {
    private static final AtomicInteger handles=new AtomicInteger();
    private File text,png;
    public static int handles() { return handles.get(); }
    @Override public boolean onCreate() {
        File directory=new File(getContext().getCacheDir(),"glory-files-probe");
        if(!directory.isDirectory() && !directory.mkdirs()) return false;
        text=new File(directory,"text"); png=new File(directory,"badge");
        try {
            try(FileOutputStream output=new FileOutputStream(text)) { output.write("中文😀 owned file\n".getBytes(StandardCharsets.UTF_8)); }
            try(InputStream input=getContext().getAssets().open("glory/badge.png"); FileOutputStream output=new FileOutputStream(png)) {
                byte[] buffer=new byte[4096]; int length,total=0;
                while((length=input.read(buffer))!=-1) { total+=length; if(total>8*1024*1024) throw new IOException("owned asset too large"); output.write(buffer,0,length); }
            }
            return true;
        } catch(IOException error) { return false; }
    }
    private File file(Uri uri) throws FileNotFoundException {
        if(!"content".equals(uri.getScheme()) || !"rs.glory.counter.files-probe".equals(uri.getAuthority()) || uri.getQuery()!=null || uri.getFragment()!=null) throw new FileNotFoundException("unknown owned fixture");
        if("/text".equals(uri.getPath())) return text;
        if("/badge".equals(uri.getPath())) return png;
        throw new FileNotFoundException("unknown owned fixture");
    }
    @Override public String getType(Uri uri) { try { return file(uri)==text ? "text/plain" : "image/png"; } catch(FileNotFoundException error) { return null; } }
    @Override public Cursor query(Uri uri,String[] projection,String selection,String[] args,String sort) {
        try {
            File source=file(uri);
            String[] columns=projection==null ? new String[]{OpenableColumns.DISPLAY_NAME,OpenableColumns.SIZE} : projection;
            MatrixCursor cursor=new MatrixCursor(columns,1); MatrixCursor.RowBuilder row=cursor.newRow();
            for(String column:columns) {
                if(OpenableColumns.DISPLAY_NAME.equals(column)) row.add(source==text ? "中文😀.txt" : "badge.png");
                else if(OpenableColumns.SIZE.equals(column)) row.add(source.length());
                else row.add(null);
            }
            return cursor;
        } catch(FileNotFoundException error) { return null; }
    }
    @Override public ParcelFileDescriptor openFile(Uri uri,String mode) throws FileNotFoundException {
        if(!"r".equals(mode)) throw new FileNotFoundException("owned read-only fixture");
        handles.incrementAndGet();
        try { return ParcelFileDescriptor.open(file(uri),ParcelFileDescriptor.MODE_READ_ONLY,new Handler(Looper.getMainLooper()),error -> handles.decrementAndGet()); }
        catch(IOException error) { handles.decrementAndGet(); throw new FileNotFoundException("owned fixture unavailable"); }
    }
    @Override public Uri insert(Uri uri,ContentValues values) { throw new UnsupportedOperationException(); }
    @Override public int update(Uri uri,ContentValues values,String selection,String[] args) { throw new UnsupportedOperationException(); }
    @Override public int delete(Uri uri,String selection,String[] args) { throw new UnsupportedOperationException(); }
}
