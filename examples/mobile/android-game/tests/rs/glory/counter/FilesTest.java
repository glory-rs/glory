package rs.glory.counter;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.BooleanSupplier;

public final class FilesTest {
    private static void check(boolean value) { if(!value) throw new AssertionError(); }
    private static void waitFor(BooleanSupplier done) throws Exception { long end=System.nanoTime()+TimeUnit.SECONDS.toNanos(5); while(!done.getAsBoolean()) { check(System.nanoTime()<end); Thread.sleep(2); } }
    private static byte[] config(int count,long per,long total,String... filters) {
        int size=32; for(String filter:filters) size+=2+filter.length();
        ByteBuffer bytes=ByteBuffer.allocate(size).order(ByteOrder.BIG_ENDIAN); bytes.putInt(1).putInt(count>1?1:0).putInt(count).putLong(per).putLong(total).putInt(filters.length);
        for(String filter:filters) bytes.putShort((short)filter.length()).put(filter.getBytes(java.nio.charset.StandardCharsets.US_ASCII));
        return bytes.array();
    }
    private static final class Main implements FilesLedger.Main,FilesLedger.Launch {
        final FilesLedger ledger; final ArrayList<Runnable> tasks=new ArrayList<>(); int code; int launches;
        Main(FilesLedger ledger) { this.ledger=ledger; }
        @Override public boolean post(Runnable task) { check(!Thread.holdsLock(ledger.lock)); tasks.add(task); return true; }
        @Override public boolean start(int code,FilesLedger.Config config) { check(!Thread.holdsLock(ledger.lock)); this.code=code; launches++; return true; }
        void run() { tasks.remove(0).run(); }
    }
    private static final class Source implements FilesLedger.Source {
        final FilesLedger ledger; final AtomicInteger closed=new AtomicInteger(),started=new AtomicInteger(); final CountDownLatch gate;
        Source(FilesLedger ledger,boolean block) { this.ledger=ledger; gate=new CountDownLatch(block?1:0); }
        @Override public byte[] read(long offset,int length) throws IOException {
            check(!Thread.holdsLock(ledger.lock)); started.incrementAndGet();
            try { if(!gate.await(5,TimeUnit.SECONDS)) throw new IOException("gate timeout"); } catch(InterruptedException error) { Thread.currentThread().interrupt(); throw new IOException(error); }
            byte[] bytes=new byte[length]; Arrays.fill(bytes,(byte)7); return bytes;
        }
        @Override public void close() { check(!Thread.holdsLock(ledger.lock)); check(closed.incrementAndGet()==1); }
    }
    private static FilesLedger.Host host(FilesLedger.Session session) { FilesLedger.Host host=session.open(); check(host!=null && host.claim() && !host.claim()); return host; }
    private static long[] selection(FilesLedger ledger,Main main,FilesLedger.Host host,long id,FilesLedger.Offered... files) throws Exception {
        check(host.beginPick(id,config(files.length,FilesLedger.MAX_BYTES,FilesLedger.MAX_BYTES))==0); main.run();
        List<FilesLedger.Opener> openers=new ArrayList<>(); for(FilesLedger.Offered file:files) openers.add(() -> file);
        ledger.result(main.code,openers); waitFor(() -> host.pollPick(id)!=0); check(host.pollPick(id)==FilesLedger.READY);
        byte[] raw=host.metadata(id); check(raw!=null && raw.length<=FilesLedger.MAX_REPLY);
        ByteBuffer bytes=ByteBuffer.wrap(raw).order(ByteOrder.BIG_ENDIAN); int count=bytes.getInt(); check(count==files.length); long[] ids=new long[count];
        for(int i=0;i<count;i++) { ids[i]=bytes.getLong(); check(bytes.getLong()==files[i].size); int n=Short.toUnsignedInt(bytes.getShort()),m=Short.toUnsignedInt(bytes.getShort()); bytes.position(bytes.position()+n+m); }
        check(!bytes.hasRemaining() && host.adopt(id) && !host.adopt(id)); return ids;
    }
    private static FilesLedger.Offered offered(Source source,String name) { return new FilesLedger.Offered(name,"text/plain",4,source); }
    private static void idle(FilesLedger ledger) throws Exception { waitFor(() -> { long[] stats=ledger.stats(); return stats[0]==0 && stats[1]==0 && stats[2]==0 && stats[3]==0 && stats[5]==0 && stats[6]==0 && stats[7]==0; }); }
    private static void pickerLifecycle() throws Exception {
        FilesLedger ledger=new FilesLedger(); Main main=new Main(ledger); FilesLedger.Session session=ledger.session(main,main); FilesLedger.Host host=host(session);
        check(host.beginPick(1,config(1,4,4))==0); host.cancelPick(1); check(ledger.stats()[0]==1);
        check(host.beginPick(2,config(1,4,4))==FilesLedger.BUSY); main.run(); check(main.launches==0 && ledger.stats()[0]==0);
        check(host.beginPick(3,config(1,4,4))==0); main.run(); int old=main.code; host.cancelPick(3); check(ledger.stats()[0]==1);
        ledger.result(old,null); check(ledger.stats()[0]==0);
        check(host.beginPick(4,config(1,4,4))==0); main.run(); check(main.code!=old); ledger.result(old,null); check(host.pollPick(4)==0);
        ledger.result(main.code,null); check(host.pollPick(4)==FilesLedger.CANCELLED); host.cancelPick(4); check(ledger.stats()[0]==0);
        session.close(); idle(ledger);
        for(boolean fail:List.of(false,true)) {
            FilesLedger.Session rejected=ledger.session(task -> { if(fail) throw new IllegalStateException(); return false; },main);
            FilesLedger.Host client=host(rejected);
            check(client.beginPick(1,config(1,4,4))==0 && client.pollPick(1)==FilesLedger.UNAVAILABLE);
            client.cancelPick(1); rejected.close(); idle(ledger);
        }
    }
    private static void readsAndRetirement() throws Exception {
        FilesLedger ledger=new FilesLedger(); Main main=new Main(ledger); FilesLedger.Session session=ledger.session(main,main); FilesLedger.Host host=host(session);
        Source source=new Source(ledger,true); long file=selection(ledger,main,host,1,offered(source,"中文😀.txt"))[0];
        for(int i=2;i<6;i++) check(host.beginRead(i,file,0,4)==0);
        waitFor(() -> source.started.get()==4); check(ledger.stats()[1]==4 && ledger.stats()[5]==4);
        for(int i=2;i<6;i++) host.cancelRead(i);
        for(int i=6;i<206;i++) check(host.beginRead(i,file,0,4)==FilesLedger.BUSY);
        host.release(file); check(source.closed.get()==0 && ledger.stats()[2]==1);
        source.gate.countDown(); idle(ledger); check(source.closed.get()==1);
        check(host.beginRead(207,file,0,1)==FilesLedger.UNAVAILABLE);
        session.close();
    }
    private static void undrainedResultsKeepGlobalSlotsAndStaleIdsNeverReplaceThem() throws Exception {
        FilesLedger ledger=new FilesLedger(); Main main=new Main(ledger); FilesLedger.Session session=ledger.session(main,main); FilesLedger.Host host=host(session),other=host(session);
        Source source=new Source(ledger,false); long file=selection(ledger,main,host,1,offered(source,"test.txt"))[0];
        for(int i=2;i<6;i++) check(host.beginRead(i,file,0,4)==0);
        waitFor(() -> ledger.stats()[5]==0); check(ledger.stats()[1]==4);
        Source second=new Source(ledger,false); long another=selection(ledger,main,other,1,offered(second,"second.txt"))[0];
        check(other.beginRead(2,another,0,4)==FilesLedger.BUSY);
        check(host.beginRead(2,file,0,4)==FilesLedger.FAILED); check(host.pollRead(2)==FilesLedger.READY);
        check(Arrays.equals(host.takeRead(2),new byte[]{7,7,7,7})); check(host.takeRead(2)==null);
        check(other.beginRead(3,another,0,4)==0); waitFor(() -> other.pollRead(3)!=0); other.cancelRead(3);
        for(int i=3;i<6;i++) host.cancelRead(i);
        session.close(); idle(ledger); check(source.closed.get()==1 && second.closed.get()==1);
    }
    private static void openingAndActivityReplacement() throws Exception {
        FilesLedger ledger=new FilesLedger(); Main main=new Main(ledger); FilesLedger.Session old=ledger.session(main,main); FilesLedger.Host host=host(old);
        CountDownLatch entered=new CountDownLatch(1),gate=new CountDownLatch(1); Source source=new Source(ledger,false);
        check(host.beginPick(1,config(1,4,4))==0); main.run(); int oldCode=main.code;
        ledger.result(oldCode,List.of(() -> { entered.countDown(); try { check(gate.await(5,TimeUnit.SECONDS)); } catch(InterruptedException error) { throw new IOException(error); } return offered(source,"late.txt"); }));
        check(entered.await(5,TimeUnit.SECONDS)); old.close(); FilesLedger.Session current=ledger.session(main,main); FilesLedger.Host next=host(current);
        check(next.beginPick(1,config(1,4,4))==FilesLedger.BUSY); check(ledger.stats()[0]==1);
        gate.countDown(); waitFor(() -> ledger.stats()[0]==0 && ledger.stats()[2]==0); check(source.closed.get()==1);
        check(next.beginPick(2,config(1,4,4))==0); main.run(); check(main.code!=oldCode); ledger.result(oldCode,null); check(next.pollPick(2)==0); ledger.result(main.code,null); next.cancelPick(2);
        current.close(); idle(ledger);
    }
    private static void budgetsAndClosingUnadoptedSources() throws Exception {
        FilesLedger ledger=new FilesLedger(); Main main=new Main(ledger); FilesLedger.Session session=ledger.session(main,main); FilesLedger.Host host=host(session);
        Source source=new Source(ledger,false); long file=selection(ledger,main,host,1,offered(source,"valid.txt"))[0];
        check(host.beginRead(2,file,-1,1)==FilesLedger.BYTES); check(host.beginRead(3,file,0,65537)==FilesLedger.BYTES); check(host.beginRead(4,file,3,2)==FilesLedger.BYTES); check(source.started.get()==0);
        Source bad=new Source(ledger,false); check(host.beginPick(5,config(1,4,4,".png"))==0); main.run(); ledger.result(main.code,List.of(() -> offered(bad,"wrong.txt")));
        waitFor(() -> host.pollPick(5)!=0 && bad.closed.get()==1); check(host.pollPick(5)==FilesLedger.TYPE); host.cancelPick(5);
        Source orphan=new Source(ledger,false); check(host.beginPick(6,config(1,4,4))==0); main.run(); ledger.result(main.code,List.of(() -> offered(orphan,"unadopted.txt"))); waitFor(() -> host.pollPick(6)==FilesLedger.READY);
        check(host.metadata(6)!=null && ledger.stats()[2]==2); host.cancelPick(6); waitFor(() -> orphan.closed.get()==1);
        for(String name:List.of("x".repeat(1025),"😀".repeat(300),"bad\ud800")) {
            Source invalid=new Source(ledger,false); long request=host.watermark+1; check(host.beginPick(request,config(1,4,4))==0); main.run(); ledger.result(main.code,List.of(() -> offered(invalid,name))); waitFor(() -> host.pollPick(request)!=0); check(host.pollPick(request)==FilesLedger.METADATA); host.cancelPick(request); waitFor(() -> invalid.closed.get()==1);
        }
        check(host.beginRead(host.watermark+1,file,0,0)==0); long read=host.watermark; waitFor(() -> host.pollRead(read)!=0); check(host.takeRead(read).length==0);
        session.close(); idle(ledger);
    }
    private static void clientsAndExhaustion() throws Exception {
        FilesLedger ledger=new FilesLedger(); Main main=new Main(ledger); FilesLedger.Session session=ledger.session(main,main); ArrayList<FilesLedger.Host> clients=new ArrayList<>();
        for(int i=0;i<32;i++) clients.add(host(session)); check(session.open()==null);
        for(FilesLedger.Host client:clients) client.close();
        for(int i=0;i<1000;i++) { FilesLedger.Host client=host(session); client.close(); }
        FilesLedger.Host last=host(session); ledger.restoreCode(0x6500); check(last.beginPick(1,config(1,4,4))==FilesLedger.FAILED); check(main.tasks.isEmpty());
        last.watermark=Long.MAX_VALUE; check(last.beginPick(Long.MAX_VALUE,config(1,4,4))==FilesLedger.FAILED); session.close(); idle(ledger);
    }
    private static void processBudgetsAndMalformedOpener() throws Exception {
        FilesLedger ledger=new FilesLedger(); Main main=new Main(ledger); FilesLedger.Session session=ledger.session(main,main); FilesLedger.Host host=host(session);
        ArrayList<Long> grants=new ArrayList<>(); ArrayList<Source> sources=new ArrayList<>();
        for(int batch=0;batch<5;batch++) {
            FilesLedger.Offered[] files=new FilesLedger.Offered[32];
            for(int i=0;i<32;i++) { Source source=new Source(ledger,false); sources.add(source); files[i]=new FilesLedger.Offered("f.txt","text/plain",0,source); }
            for(long id:selection(ledger,main,host,batch+1,files)) grants.add(id);
        }
        check(ledger.stats()[2]==160); AtomicInteger opened=new AtomicInteger();
        check(host.beginPick(6,config(1,4,4))==0); main.run(); ledger.result(main.code,List.of(() -> { opened.incrementAndGet(); return null; }));
        waitFor(() -> host.pollPick(6)!=0); check(host.pollPick(6)==FilesLedger.BUSY && opened.get()==0); host.cancelPick(6);
        for(long grant:grants) host.release(grant); idle(ledger); for(Source source:sources) check(source.closed.get()==1);
        for(long id:List.of(7L,8L)) {
            check(host.beginPick(id,config(1,4,4))==0); main.run();
            ledger.result(main.code,List.of(() -> id==7 ? null : new FilesLedger.Offered("bad.txt","text/plain",0,null)));
            waitFor(() -> host.pollPick(id)!=0); check(host.pollPick(id)==FilesLedger.FAILED); host.cancelPick(id); idle(ledger);
        }
        for(int i=0;i<2;i++) selection(ledger,main,host,9+i,new FilesLedger.Offered("large.txt","text/plain",FilesLedger.MAX_BYTES,new Source(ledger,false)));
        check(ledger.stats()[7]==2*FilesLedger.MAX_BYTES);
        Source excess=new Source(ledger,false); check(host.beginPick(11,config(1,1,1))==0); main.run();
        ledger.result(main.code,List.of(() -> new FilesLedger.Offered("overflow.txt","text/plain",1,excess)));
        waitFor(() -> host.pollPick(11)!=0 && excess.closed.get()==1); check(host.pollPick(11)==FilesLedger.BYTES); host.cancelPick(11);
        session.close(); idle(ledger);

        FilesLedger metadataLedger=new FilesLedger(); Main metadataMain=new Main(metadataLedger); FilesLedger.Session metadataSession=metadataLedger.session(metadataMain,metadataMain); FilesLedger.Host metadataHost=host(metadataSession);
        String name="x".repeat(1024),type="application/"+"x".repeat(115);
        for(int batch=0;batch<5;batch++) {
            FilesLedger.Offered[] files=new FilesLedger.Offered[28];
            for(int i=0;i<28;i++) files[i]=new FilesLedger.Offered(name,type,0,new Source(metadataLedger,false));
            selection(metadataLedger,metadataMain,metadataHost,batch+1,files);
        }
        long previous=metadataLedger.stats()[6]; check(previous==140*(1024+127));
        Source[] extra={new Source(metadataLedger,false),new Source(metadataLedger,false),new Source(metadataLedger,false)};
        check(metadataHost.beginPick(6,config(3,4,4))==0); metadataMain.run(); List<FilesLedger.Opener> openers=new ArrayList<>();
        for(Source source:extra) openers.add(() -> new FilesLedger.Offered(name,type,0,source));
        metadataLedger.result(metadataMain.code,openers); waitFor(() -> metadataHost.pollPick(6)!=0); check(metadataHost.pollPick(6)==FilesLedger.METADATA); metadataHost.cancelPick(6);
        waitFor(() -> metadataLedger.stats()[2]==140); check(metadataLedger.stats()[6]==previous); for(Source source:extra) check(source.closed.get()==1);
        metadataSession.close(); idle(metadataLedger);
    }
    public static void main(String[] args) throws Exception {
        pickerLifecycle(); readsAndRetirement(); undrainedResultsKeepGlobalSlotsAndStaleIdsNeverReplaceThem(); openingAndActivityReplacement(); budgetsAndClosingUnadoptedSources(); clientsAndExhaustion(); processBudgetsAndMalformedOpener();
        System.out.println("Android files ledger: lazy picker, physical/global admission, adopt/cancel, exact reads, pending results, lifecycle, UTF-8/budgets, cleanup and exhaustion passed");
    }
}
