package rs.glory.counter;

import java.util.concurrent.atomic.AtomicLongArray;

// Only files-ui-probe compiles this finite counter set. No URIs, paths or bodies.
public final class GlorySystemFilesTrace {
    private static final AtomicLongArray counts=new AtomicLongArray(8);
    private GlorySystemFilesTrace() {}
    public static void mark(int index) {
        if(index<0 || index>=8) throw new IllegalArgumentException("trace index");
        counts.updateAndGet(index,value -> value==Long.MAX_VALUE ? value : value+1);
    }
    public static long[] snapshot() {
        long[] values=new long[8];
        for(int i=0;i<8;i++) values[i]=counts.get(i);
        return values;
    }
}
