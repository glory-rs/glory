package rs.glory.counter;

import java.lang.ref.WeakReference;
import java.util.ArrayList;
import java.util.Arrays;

// Pure bounded request ownership, shared by the platform host and JVM tests.
final class ClipboardLedger {
    static final long PENDING=0, WRITTEN=1, TEXT=2, UNAVAILABLE=-1, DENIED=-2,
        FAILED=-3, BUSY=-4, TOO_LONG=-5, INVALID_LIMITS=-6;
    static final int MAX_BYTES=65536, MAX_HOSTS=32;
    interface Scheduler { boolean post(Runnable work); }
    interface Backend { Reply execute(byte[] write, int limit); }
    static final class Reply {
        final long code; final byte[] data;
        Reply(long code, byte[] data) { this.code=code; this.data=data; }
        static Reply code(long code) { return new Reply(code, null); }
    }
    private static final class Record {
        final WeakReference<Host> host;
        final long id; final int limit; final boolean reading;
        byte[] write; Reply reply; boolean retired;
        Record(Host host,long id,byte[] write,int limit) {
            this.host=new WeakReference<>(host); this.id=id; this.write=write;
            this.reading=write==null; this.limit=limit;
        }
    }
    final Scheduler scheduler; final Backend backend;
    final ArrayList<WeakReference<Host>> hosts=new ArrayList<>();
    private Record active;
    private boolean closed;
    ClipboardLedger(Scheduler scheduler,Backend backend) { this.scheduler=scheduler; this.backend=backend; }
    synchronized Host open() {
        hosts.removeIf(ref -> { Host host=ref.get(); return host==null || host.closed; });
        if(closed || hosts.size()==MAX_HOSTS) return null;
        Host host=new Host(this); hosts.add(new WeakReference<>(host)); return host;
    }
    private void run(Record record) {
        byte[] write;
        synchronized(this) {
            Host host=record.host.get();
            if(closed || record.retired || host==null || host.closed || host.current!=record) {
                if(active==record) active=null;
                record.write=null; return;
            }
            write=record.write; record.write=null;
        }
        Reply reply;
        try {
            reply=backend.execute(write,record.limit);
            if(reply==null || reply.code==PENDING
                || reply.code==TEXT && (!record.reading || reply.data==null)
                || reply.code==WRITTEN && record.reading) reply=Reply.code(FAILED);
            else if(reply.data!=null && reply.data.length>record.limit) reply=Reply.code(TOO_LONG);
        } catch(RuntimeException error) { reply=Reply.code(FAILED); }
        synchronized(this) {
            if(active==record) active=null;
            Host host=record.host.get();
            if(!closed && !record.retired && host!=null && !host.closed && host.current==record) record.reply=reply;
        }
    }
    synchronized void close() {
        if(closed) return;
        closed=true;
        while(!hosts.isEmpty()) { Host host=hosts.remove(hosts.size()-1).get(); if(host!=null) host.close(); }
        // A queued/running record owns admission until run actually settles.
    }
    static final class Host {
        final ClipboardLedger ledger;
        private Record current; private long last; private boolean closed,claimed;
        Host(ClipboardLedger ledger) { this.ledger=ledger; }
        public long protocolVersion() { return 1; }
        public boolean claim() {
            synchronized(ledger) {
                if(closed || ledger.closed || claimed) return false;
                claimed=true; return true;
            }
        }
        public long begin(long id,byte[] write,long limit) {
            Record record;
            synchronized(ledger) {
                if(closed || ledger.closed) return UNAVAILABLE;
                if(id<=0 || id<=last) return FAILED;
                if(limit<=0 || limit>MAX_BYTES) return INVALID_LIMITS;
                if(write!=null && write.length>MAX_BYTES) return TOO_LONG;
                if(current!=null || ledger.active!=null) return BUSY;
                last=id;
                record=new Record(this,id,write==null?null:write.clone(),(int)limit);
                current=record; ledger.active=record;
            }
            boolean posted;
            try { posted=ledger.scheduler.post(() -> ledger.run(record)); }
            catch(RuntimeException error) { posted=false; }
            if(!posted) synchronized(ledger) {
                if(ledger.active==record) ledger.active=null;
                record.write=null;
                if(current==record && !closed && !ledger.closed) record.reply=Reply.code(UNAVAILABLE);
            }
            return PENDING;
        }
        public long poll(long id) {
            synchronized(ledger) {
                if(closed || ledger.closed || current==null || current.id!=id) return UNAVAILABLE;
                if(current.reply==null) return PENDING;
                long code=current.reply.code;
                if(code!=TEXT) current=null;
                return code;
            }
        }
        public byte[] take(long id) {
            synchronized(ledger) {
                if(closed || ledger.closed || current==null || current.id!=id
                    || current.reply==null || current.reply.code!=TEXT) return null;
                byte[] data=current.reply.data; current=null; return data;
            }
        }
        public void cancel(long id) {
            synchronized(ledger) {
                if(current!=null && current.id==id) { current.retired=true; current=null; }
            }
        }
        public void close() {
            synchronized(ledger) {
                if(closed) return;
                closed=true;
                if(current!=null) { current.retired=true; current=null; }
                ledger.hosts.removeIf(ref -> ref.get()==null || ref.get()==this);
            }
        }
        public long stats() {
            synchronized(ledger) { return (current==null?0:1) | (ledger.active==null?0:0x10000); }
        }
    }
    static final class Budget extends Exception { private static final long serialVersionUID=1; }
    // No toString/coercion of an ambient URI or unbounded UTF-8 allocation.
    // Check raw UTF-8 and canonical payload budgets independently. Java's
    // unpaired UTF-16 surrogates are explicitly U+FFFD, as in native editing.
    static byte[] encode(CharSequence text,int limit) throws Budget {
        if(limit<=0 || limit>MAX_BYTES || text.length()>limit) throw new Budget();
        int length=text.length(),raw=0,size=0;
        byte[] bytes=new byte[limit];
        for(int i=0;i<length;i++) {
            int c=text.charAt(i);
            if(Character.isHighSurrogate((char)c) && i+1<length && Character.isLowSurrogate(text.charAt(i+1))) c=Character.toCodePoint((char)c,text.charAt(++i));
            else if(Character.isSurrogate((char)c)) c=0xfffd;
            int width=c<0x80?1:c<0x800?2:c<0x10000?3:4;
            raw+=width;
            if(c=='\r') {
                c='\n';
                if(i+1<length && text.charAt(i+1)=='\n') { i++; raw++; }
            } else if(c==0) c=0xfffd;
            width=c<0x80?1:c<0x800?2:c<0x10000?3:4;
            if(raw>limit || size+width>limit) throw new Budget();
            if(width==1) bytes[size++]=(byte)c;
            else {
                if(width==2) bytes[size++]=(byte)(0xc0|(c>>6));
                else if(width==3) { bytes[size++]=(byte)(0xe0|(c>>12)); bytes[size++]=(byte)(0x80|((c>>6)&63)); }
                else { bytes[size++]=(byte)(0xf0|(c>>18)); bytes[size++]=(byte)(0x80|((c>>12)&63)); bytes[size++]=(byte)(0x80|((c>>6)&63)); }
                bytes[size++]=(byte)(0x80|(c&63));
            }
        }
        return Arrays.copyOf(bytes,size);
    }
}
