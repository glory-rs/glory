package rs.glory.counter;

import java.io.IOException;
import java.lang.ref.WeakReference;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.CharBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.ThreadPoolExecutor;
import java.util.concurrent.TimeUnit;

// App-owned process-lifetime OS admission, not a UI/controller/history cache.
// Sessions hold Activities only through their launcher's weak reference.
final class FilesLedger {
    static final long PENDING=0, CANCELLED=1, READY=2;
    static final long COUNT=-1, TYPE=-2, BYTES=-3, METADATA=-4, BUSY=-5, UNAVAILABLE=-6, FAILED=-7;
    static final int MAX_HANDLES=160, MAX_METADATA=32768, MAX_REPLY=33412;
    static final long MAX_BYTES=1L<<30;
    interface Main { boolean post(Runnable task); }
    interface Launch { boolean start(int code, Config config); }
    interface Source { byte[] read(long offset,int length) throws IOException; void close() throws IOException; }
    interface Opener { Offered open() throws IOException; }
    static final class Offered {
        final String name, type; final long size; final Source source;
        Offered(String name,String type,long size,Source source) { this.name=name; this.type=type; this.size=size; this.source=source; }
    }
    static final class Refusal extends IOException {
        private static final long serialVersionUID=1L;
        final long code;
        Refusal(long code) { super("file service refusal"); this.code=code; }
    }
    static final class Config {
        final boolean multiple; final int count; final long fileBytes,totalBytes; final List<String> filters;
        Config(boolean multiple,int count,long fileBytes,long totalBytes,List<String> filters) throws Refusal {
            if(count<1 || count>32 || !multiple && count!=1 || fileBytes<0 || fileBytes>MAX_BYTES || totalBytes<0 || totalBytes>MAX_BYTES || filters.size()>32) throw new Refusal(FAILED);
            int total=0;
            for(String filter:filters) {
                if(filter==null || filter.length()>127 || !filter.matches("\\.[a-z0-9._-]{1,31}|[a-z0-9!#$&^_.+\\-]+/(?:[a-z0-9!#$&^_.+\\-]+|\\*)")) throw new Refusal(TYPE);
                total+=filter.length();
            }
            if(total>2048) throw new Refusal(METADATA);
            this.multiple=multiple; this.count=count; this.fileBytes=fileBytes; this.totalBytes=totalBytes; this.filters=new ArrayList<>(filters);
        }
        static Config parse(byte[] bytes) throws Refusal {
            if(bytes==null || bytes.length<32 || bytes.length>4096) throw new Refusal(FAILED);
            try {
                ByteBuffer input=ByteBuffer.wrap(bytes).order(ByteOrder.BIG_ENDIAN);
                if(input.getInt()!=1) throw new Refusal(FAILED);
                int flags=input.getInt(), count=input.getInt(); long file=input.getLong(),total=input.getLong(); int terms=input.getInt();
                if((flags&~1)!=0 || terms<0 || terms>32) throw new Refusal(FAILED);
                List<String> filters=new ArrayList<>();
                for(int i=0;i<terms;i++) {
                    int length=Short.toUnsignedInt(input.getShort());
                    if(length>127 || length>input.remaining()) throw new Refusal(METADATA);
                    byte[] text=new byte[length]; input.get(text);
                    filters.add(StandardCharsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(text)).toString());
                }
                if(input.hasRemaining()) throw new Refusal(FAILED);
                return new Config(flags==1,count,file,total,filters);
            } catch(java.nio.BufferUnderflowException | CharacterCodingException error) { throw new Refusal(FAILED); }
        }
        boolean matches(String name,String type) {
            if(filters.isEmpty()) return true;
            for(String filter:filters) {
                if(filter.startsWith(".")) { if(name.toLowerCase(Locale.ROOT).endsWith(filter)) return true; }
                else if(filter.endsWith("/*")) { if(type.startsWith(filter.substring(0,filter.length()-1))) return true; }
                else if(filter.equals(type)) return true;
            }
            return false;
        }
    }
    static final class Session {
        final FilesLedger ledger; final Main main; final Launch launch; boolean closed;
        Session(FilesLedger ledger,Main main,Launch launch) { this.ledger=ledger; this.main=main; this.launch=launch; }
        Host open() { return ledger.open(this); }
        void close() { ledger.closeSession(this); }
    }
    private static final class Pick {
        final WeakReference<Host> host; final long id; final int code; final Config config;
        int phase; // 0 queued, 1 native prompt, 2 opening, 3 ready
        long result; List<Grant> files; byte[] metadata;
        Pick(Host host,long id,int code,Config config) { this.host=new WeakReference<>(host); this.id=id; this.code=code; this.config=config; }
    }
    private static final class Grant {
        final long id,size; final byte[] name,type; final Source source; final WeakReference<Host> owner;
        int readers; boolean retired,closing,pending=true;
        Grant(long id,Offered offered,byte[] name,byte[] type,WeakReference<Host> owner) { this.id=id; size=offered.size; source=offered.source; this.name=name; this.type=type; this.owner=owner; }
    }
    private static final class Read {
        final WeakReference<Host> host; final long id; final Grant grant; final long offset; final int length;
        long result; byte[] bytes; boolean running=true;
        Read(Host host,long id,Grant grant,long offset,int length) { this.host=new WeakReference<>(host); this.id=id; this.grant=grant; this.offset=offset; this.length=length; }
    }
    final Object lock=new Object();
    private final ArrayList<WeakReference<Host>> hosts=new ArrayList<>();
    private final ThreadPoolExecutor workers=new ThreadPoolExecutor(5,5,1,TimeUnit.SECONDS,new ArrayBlockingQueue<>(165), task -> { Thread t=new Thread(task,"glory-files-io"); t.setDaemon(true); return t; });
    private final ArrayList<Read> readFlights=new ArrayList<>();
    private final LinkedHashMap<Long,Grant> allGrants=new LinkedHashMap<>();
    private long declaredBytes; private int metadataBytes,activeReads;
    private Pick picker; private int handles,jobs; private long nextGrant=1; private int nextCode=0x5000;
    FilesLedger() { workers.allowCoreThreadTimeOut(true); }
    Session session(Main main,Launch launch) { return new Session(this,main,launch); }
    int nextCode() { synchronized(lock) { return nextCode; } }
    void restoreCode(int code) { synchronized(lock) { if(code>=0x5000 && code<=0x6500) nextCode=Math.max(nextCode,code); } }
    private Host open(Session session) {
        List<Grant> close=new ArrayList<>(); Host result=null;
        synchronized(lock) {
            hosts.removeIf(ref -> { Host host=ref.get(); return host==null || host.closed; });
            for(Grant grant:allGrants.values()) if(!grant.pending && grant.owner.get()==null) retire(grant,close);
            readFlights.removeIf(read -> !read.running && read.host.get()==null);
            if(picker!=null && picker.phase==3 && picker.host.get()==null) { if(picker.files!=null) for(Grant grant:picker.files) retire(grant,close); picker=null; }
            if(!session.closed && hosts.size()<32) { result=new Host(session); hosts.add(new WeakReference<>(result)); }
        }
        closeLater(close); return result;
    }
    private boolean live(Host host,Pick pick) { return host!=null && !host.closed && !host.session.closed && host.pick==pick; }
    private void submit(Runnable task) {
        // Every reservation occurs before enqueue, and teardown never shuts this
        // process-lifetime pool down. <=160 closes + one opener + four reads.
        workers.execute(() -> { try { task.run(); } finally { synchronized(lock) { jobs--; } } });
    }
    private void launch(Pick ticket) {
        Host host=ticket.host.get();
        synchronized(lock) {
            if(picker!=ticket) return;
            if(!live(host,ticket)) { picker=null; return; }
            ticket.phase=1;
        }
        boolean launched=false;
        try { launched=host.session.launch.start(ticket.code,ticket.config); }
        catch(RuntimeException error) { /* native launch failed */ }
        if(!launched) finishPrompt(ticket,UNAVAILABLE);
    }
    private void finishPrompt(Pick ticket,long result) {
        synchronized(lock) {
            if(picker!=ticket) return;
            Host host=ticket.host.get();
            if(live(host,ticket)) { ticket.result=result; ticket.phase=3; }
            else picker=null;
        }
    }
    boolean owns(int code) { synchronized(lock) { return picker!=null && picker.code==code && picker.phase==1; } }
    Config wanted(int code) { synchronized(lock) { if(!owns(code)) return null; return live(picker.host.get(),picker) ? picker.config : null; } }
    void failResult(int code,long result) { Pick ticket; synchronized(lock) { if(!owns(code)) return; ticket=picker; } finishPrompt(ticket,result); }
    void result(int code,List<Opener> openers) {
        Pick ticket;
        synchronized(lock) {
            if(!owns(code)) return; ticket=picker;
            if(!live(ticket.host.get(),ticket)) { picker=null; return; }
            if(openers==null) { ticket.result=CANCELLED; ticket.phase=3; return; }
            if(openers.isEmpty() || openers.size()>ticket.config.count) { ticket.result=COUNT; ticket.phase=3; return; }
            ticket.phase=2; jobs++;
        }
        List<Opener> snapshot=new ArrayList<>(openers);
        submit(() -> open(ticket,snapshot));
    }
    private static byte[] text(String value,int limit,boolean name) throws Refusal {
        if(value==null || value.length()>limit) throw new Refusal(METADATA);
        if(name) value=value.replace("\r\n","\n").replace('\r','\n').replace('\0','\ufffd');
        else value=value.toLowerCase(Locale.ROOT);
        if(name && value.isEmpty() || !name && !value.isEmpty() && !value.matches("[a-z0-9!#$&^_.+\\-]+/[a-z0-9!#$&^_.+\\-]+")) throw new Refusal(METADATA);
        try {
            ByteBuffer buffer=StandardCharsets.UTF_8.newEncoder().onMalformedInput(CodingErrorAction.REPORT).onUnmappableCharacter(CodingErrorAction.REPORT).encode(CharBuffer.wrap(value));
            if(buffer.remaining()>limit) throw new Refusal(METADATA);
            byte[] bytes=new byte[buffer.remaining()]; buffer.get(bytes); return bytes;
        } catch(CharacterCodingException error) { throw new Refusal(METADATA); }
    }
    private void open(Pick ticket,List<Opener> openers) {
        List<Grant> selected=new ArrayList<>(); long failure=READY,total=0; int metadata=0;
        try {
            for(Opener opener:openers) {
                long id;
                synchronized(lock) {
                    if(!live(ticket.host.get(),ticket)) throw new Refusal(UNAVAILABLE);
                    if(handles>=MAX_HANDLES) throw new Refusal(BUSY);
                    if(nextGrant==Long.MAX_VALUE) throw new Refusal(FAILED);
                    id=nextGrant++; handles++;
                }
                Offered offered=null;
                try {
                    offered=opener.open();
                    if(offered==null || offered.source==null) throw new Refusal(FAILED);
                    byte[] name=text(offered.name,1024,true),type=text(offered.type,127,false);
                    if(offered.size<0 || offered.size>ticket.config.fileBytes || offered.size>MAX_BYTES) throw new Refusal(BYTES);
                    total=Math.addExact(total,offered.size); metadata+=name.length+type.length;
                    if(total>ticket.config.totalBytes) throw new Refusal(BYTES);
                    if(metadata>MAX_METADATA) throw new Refusal(METADATA);
                    if(!ticket.config.matches(new String(name,StandardCharsets.UTF_8),new String(type,StandardCharsets.UTF_8))) throw new Refusal(TYPE);
                    Grant grant=new Grant(id,offered,name,type,ticket.host);
                    synchronized(lock) {
                        if(declaredBytes>2*MAX_BYTES-offered.size) throw new Refusal(BYTES);
                        if(metadataBytes>160*1024-name.length-type.length) throw new Refusal(METADATA);
                        allGrants.put(id,grant); declaredBytes+=offered.size; metadataBytes+=name.length+type.length;
                    }
                    selected.add(grant); offered=null;
                } finally {
                    if(offered!=null) { try { if(offered.source!=null) offered.source.close(); } catch(IOException | RuntimeException ignored) {} finally { synchronized(lock) { handles--; } } }
                    // An opener must close partial resources before throwing.
                    else if(selected.isEmpty() || selected.get(selected.size()-1).id!=id) { synchronized(lock) { handles--; } }
                }
            }
        } catch(Refusal error) { failure=error.code; }
        catch(IOException | RuntimeException error) { failure=FAILED; }
        boolean retain;
        synchronized(lock) {
            retain=picker==ticket && live(ticket.host.get(),ticket) && failure==READY;
            if(picker==ticket) {
                if(live(ticket.host.get(),ticket)) { ticket.phase=3; ticket.result=failure; if(retain) { ticket.files=selected; ticket.metadata=metadata(selected); } }
                else picker=null;
            }
        }
        if(!retain) for(Grant grant:selected) closeSource(grant);
    }
    private static byte[] metadata(List<Grant> grants) {
        int length=4; for(Grant grant:grants) length+=20+grant.name.length+grant.type.length;
        ByteBuffer bytes=ByteBuffer.allocate(length).order(ByteOrder.BIG_ENDIAN); bytes.putInt(grants.size());
        for(Grant grant:grants) bytes.putLong(grant.id).putLong(grant.size).putShort((short)grant.name.length).putShort((short)grant.type.length).put(grant.name).put(grant.type);
        return bytes.array();
    }
    private void closeSource(Grant grant) {
        try { grant.source.close(); } catch(IOException | RuntimeException ignored) {}
        finally { synchronized(lock) { handles--; allGrants.remove(grant.id); metadataBytes-=grant.name.length+grant.type.length; declaredBytes-=grant.size; } }
    }
    private void retire(Grant grant,List<Grant> close) {
        grant.retired=true;
        if(grant.readers==0 && !grant.closing) { grant.closing=true; close.add(grant); jobs++; }
    }
    private void closeLater(List<Grant> close) { for(Grant grant:close) submit(() -> closeSource(grant)); }
    private void read(Read read) {
        byte[] bytes=null; long status=FAILED;
        try {
            boolean consume;
            synchronized(lock) { Host host=read.host.get(); consume=host!=null && !host.closed && !host.session.closed && host.reads.get(read.id)==read && !read.grant.retired; }
            if(consume) { bytes=read.grant.source.read(read.offset,read.length); if(bytes!=null && bytes.length==read.length) status=READY; else bytes=null; }
        } catch(IOException | RuntimeException error) { bytes=null; }
        List<Grant> close=new ArrayList<>();
        synchronized(lock) {
            read.running=false; activeReads--; read.grant.readers--;
            Host host=read.host.get();
            if(host!=null && !host.closed && !host.session.closed && host.reads.get(read.id)==read && !read.grant.retired) { read.result=status; read.bytes=bytes; }
            else { readFlights.remove(read); if(host!=null) host.reads.remove(read.id); }
            if(read.grant.retired) retire(read.grant,close);
        }
        closeLater(close);
    }
    private void closeHost(Host host,List<Grant> close) {
        host.closed=true;
        if(host.pick!=null) { Pick pick=host.pick; host.pick=null; if(pick.phase==3) { if(pick.files!=null) for(Grant grant:pick.files) retire(grant,close); if(picker==pick) picker=null; } }
        for(Read read:host.reads.values()) if(!read.running) readFlights.remove(read);
        host.reads.clear();
        for(Grant grant:host.grants.values()) retire(grant,close);
        host.grants.clear();
    }
    private void closeSession(Session session) {
        List<Grant> close=new ArrayList<>();
        synchronized(lock) {
            session.closed=true;
            for(WeakReference<Host> ref:hosts) { Host host=ref.get(); if(host!=null && host.session==session) closeHost(host,close); }
            hosts.removeIf(ref -> { Host host=ref.get(); return host==null || host.closed; });
        }
        closeLater(close);
    }
    long[] stats() { synchronized(lock) { return new long[]{picker==null?0:1,readFlights.size(),handles,jobs,hosts.size(),activeReads,metadataBytes,declaredBytes}; } }
    static final class Host {
        final Session session; Pick pick; boolean claimed,closed; long watermark;
        final LinkedHashMap<Long,Grant> grants=new LinkedHashMap<>();
        final LinkedHashMap<Long,Read> reads=new LinkedHashMap<>();
        Host(Session session) { this.session=session; }
        public long protocolVersion() { return 1; }
        public boolean claim() { synchronized(session.ledger.lock) { if(claimed || closed || session.closed) return false; claimed=true; return true; } }
        private boolean id(long id) { if(id<=watermark || id<=0) return false; watermark=id; return true; }
        public long beginPick(long id,byte[] bytes) {
            FilesLedger ledger=session.ledger; Config config;
            try { config=Config.parse(bytes); } catch(Refusal error) { return error.code; }
            Pick ticket;
            synchronized(ledger.lock) {
                if(closed || session.closed || !claimed) return UNAVAILABLE;
                if(!id(id)) return FAILED;
                if(pick!=null || ledger.picker!=null) return BUSY;
                if(ledger.nextCode>=0x6500) return FAILED;
                ticket=new Pick(this,id,ledger.nextCode++,config); pick=ticket; ledger.picker=ticket;
            }
            boolean queued=false;
            try { queued=session.main.post(() -> ledger.launch(ticket)); }
            catch(RuntimeException error) { /* no native operation was accepted */ }
            if(!queued) ledger.finishPrompt(ticket,UNAVAILABLE);
            return PENDING;
        }
        public long pollPick(long id) { synchronized(session.ledger.lock) { return closed || session.closed || pick==null || pick.id!=id ? UNAVAILABLE : pick.result; } }
        public byte[] metadata(long id) { synchronized(session.ledger.lock) { return pick!=null && pick.id==id && pick.phase==3 && pick.result==READY ? pick.metadata : null; } }
        public boolean adopt(long id) {
            FilesLedger ledger=session.ledger;
            synchronized(ledger.lock) {
                if(closed || session.closed || pick==null || pick.id!=id || pick.phase!=3 || pick.result!=READY || pick.files==null) return false;
                for(Grant grant:pick.files) { grant.pending=false; grants.put(grant.id,grant); }
                if(ledger.picker==pick) ledger.picker=null;
                pick=null; return true;
            }
        }
        public void cancelPick(long id) {
            FilesLedger ledger=session.ledger; List<Grant> close=new ArrayList<>();
            synchronized(ledger.lock) {
                if(pick==null || pick.id!=id) return;
                Pick previous=pick; pick=null;
                if(previous.phase==3) { if(previous.files!=null) for(Grant grant:previous.files) ledger.retire(grant,close); if(ledger.picker==previous) ledger.picker=null; }
            }
            ledger.closeLater(close);
        }
        public long beginRead(long id,long file,long offset,long length) {
            FilesLedger ledger=session.ledger; Read read;
            synchronized(ledger.lock) {
                if(closed || session.closed || !claimed) return UNAVAILABLE;
                if(!id(id)) return FAILED;
                Grant grant=grants.get(file);
                if(grant==null || grant.retired) return UNAVAILABLE;
                if(offset<0 || length<0 || length>65536 || offset>grant.size || length>grant.size-offset) return BYTES;
                if(ledger.readFlights.size()>=4 || reads.size()>=4) return BUSY;
                read=new Read(this,id,grant,offset,(int)length); reads.put(id,read); ledger.readFlights.add(read); ledger.activeReads++; grant.readers++; ledger.jobs++;
            }
            ledger.submit(() -> ledger.read(read)); return PENDING;
        }
        public long pollRead(long id) { synchronized(session.ledger.lock) { Read read=reads.get(id); return closed || session.closed || read==null ? UNAVAILABLE : read.result; } }
        public byte[] takeRead(long id) { synchronized(session.ledger.lock) { Read read=reads.get(id); if(read==null || read.result!=READY) return null; reads.remove(id); session.ledger.readFlights.remove(read); return read.bytes; } }
        public void cancelRead(long id) { synchronized(session.ledger.lock) { Read read=reads.remove(id); if(read!=null && !read.running) session.ledger.readFlights.remove(read); } }
        public void release(long file) {
            FilesLedger ledger=session.ledger; List<Grant> close=new ArrayList<>();
            synchronized(ledger.lock) {
                Grant grant=grants.remove(file);
                if(grant!=null) {
                    reads.values().removeIf(read -> { if(read.grant!=grant) return false; if(!read.running) ledger.readFlights.remove(read); return true; });
                    ledger.retire(grant,close);
                }
            }
            ledger.closeLater(close);
        }
        public void close() { FilesLedger ledger=session.ledger; List<Grant> close=new ArrayList<>(); synchronized(ledger.lock) { ledger.closeHost(this,close); } ledger.closeLater(close); }
        public long[] stats() { return session.ledger.stats(); }
    }
}
