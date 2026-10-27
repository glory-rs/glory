package rs.glory.counter;

import java.nio.charset.StandardCharsets;
import java.util.ArrayDeque;
import java.util.Arrays;
import java.util.concurrent.CountDownLatch;

public final class ClipboardTest {
    static void check(boolean value) { if(!value) throw new AssertionError("Clipboard ownership invariant"); }
    static final class Queue implements ClipboardLedger.Scheduler {
        final ArrayDeque<Runnable> jobs=new ArrayDeque<>();
        public boolean post(Runnable work) { jobs.add(work); check(jobs.size()==1); return true; }
        void run() { jobs.remove().run(); }
    }
    static ClipboardLedger.Reply written() { return ClipboardLedger.Reply.code(ClipboardLedger.WRITTEN); }
    static void budgets() throws Exception {
        for(String value:new String[]{"", "A中😀Z", "\ud800", "a\r\nb\rc\0"}) {
            String expected=value.replace("\ud800","�").replace("\r\n","\n").replace('\r','\n').replace("\0","�");
            check(new String(ClipboardLedger.encode(value,128),StandardCharsets.UTF_8).equals(expected));
        }
        for(String value:new String[]{"😀", "\0", "\udc00", "\r\n"}) {
            try { ClipboardLedger.encode(value,1); throw new AssertionError("budget accepted"); } catch(ClipboardLedger.Budget expected) {}
        }
        check(ClipboardLedger.encode("x".repeat(65536),65536).length==65536);
        try { ClipboardLedger.encode("x".repeat(65537),65536); throw new AssertionError(); } catch(ClipboardLedger.Budget expected) {}
    }
    static void cancellation() {
        Queue queue=new Queue(); int[] calls={0};
        ClipboardLedger ledger=new ClipboardLedger(queue,(write,limit)->{ calls[0]++; return written(); });
        ClipboardLedger.Host first=ledger.open(), second=ledger.open();
        check(first.claim()); check(!first.claim());
        check(first.begin(1,new byte[]{1},65536)==0); first.cancel(1);
        for(int i=0;i<200;i++) check(second.begin(i+1,new byte[0],65536)==ClipboardLedger.BUSY);
        check((first.stats()&0x10000)!=0); queue.run(); check(calls[0]==0);
        check(first.poll(1)==ClipboardLedger.UNAVAILABLE);
        check(first.begin(1,new byte[0],65536)==ClipboardLedger.FAILED);
        check(second.begin(201,new byte[0],65536)==0); queue.run(); check(calls[0]==1);
        check(second.poll(201)==ClipboardLedger.WRITTEN); check(second.poll(201)==ClipboardLedger.UNAVAILABLE);
    }
    static void runningCancellation() throws Exception {
        Queue queue=new Queue(); CountDownLatch entered=new CountDownLatch(1), exit=new CountDownLatch(1);
        ClipboardLedger ledger=new ClipboardLedger(queue,(write,limit)->{
            entered.countDown(); try { check(exit.await(5,java.util.concurrent.TimeUnit.SECONDS)); } catch(InterruptedException error) { throw new AssertionError(error); } return written();
        });
        ClipboardLedger.Host host=ledger.open(); check(host.begin(1,new byte[0],65536)==0);
        Thread worker=new Thread(queue.jobs.remove()); worker.start(); check(entered.await(5,java.util.concurrent.TimeUnit.SECONDS)); host.cancel(1);
        check(host.begin(2,new byte[0],65536)==ClipboardLedger.BUSY); check((host.stats()&0x10000)!=0);
        exit.countDown(); worker.join(5000); check(!worker.isAlive()); check(host.stats()==0); check(host.poll(1)==ClipboardLedger.UNAVAILABLE);
    }
    static void lifecycleAndBodies() {
        Queue queue=new Queue(); byte[][] submitted={null};
        ClipboardLedger ledger=new ClipboardLedger(queue,(write,limit)->{
            submitted[0]=write;
            return write==null?new ClipboardLedger.Reply(ClipboardLedger.TEXT,new byte[]{1,2,3}):written();
        });
        ClipboardLedger.Host host=ledger.open(); byte[] text={7};
        check(host.begin(1,text,65536)==0); text[0]=8; queue.run(); check(submitted[0][0]==7); check(host.poll(1)==1);
        check(host.begin(2,null,2)==0); queue.run(); check(host.poll(2)==ClipboardLedger.TOO_LONG);
        check(host.begin(3,null,3)==0); queue.run(); check(host.poll(3)==2); check(host.poll(3)==2);
        check(Arrays.equals(host.take(3),new byte[]{1,2,3})); check(host.take(3)==null);
        check(host.begin(Long.MAX_VALUE,new byte[0],65536)==0); queue.run(); check(host.poll(Long.MAX_VALUE)==1);
        check(host.begin(Long.MAX_VALUE,new byte[0],65536)==ClipboardLedger.FAILED);
        host.close(); check(host.begin(4,null,3)==ClipboardLedger.UNAVAILABLE);
        java.util.ArrayList<ClipboardLedger.Host> retained=new java.util.ArrayList<>();
        for(int i=0;i<ClipboardLedger.MAX_HOSTS;i++) { ClipboardLedger.Host item=ledger.open(); check(item!=null); retained.add(item); }
        check(ledger.open()==null); ledger.close(); check(ledger.open()==null);
    }
    static void closeAndPostFailure() {
        Queue queue=new Queue(); int[] calls={0};
        ClipboardLedger ledger=new ClipboardLedger(queue,(write,limit)->{ calls[0]++; return written(); });
        ClipboardLedger.Host host=ledger.open(); host.begin(1,new byte[0],65536); ledger.close();
        check((host.stats()&0x10000)!=0); queue.run(); check(calls[0]==0 && host.stats()==0);
        ClipboardLedger failed=new ClipboardLedger(work->false,(write,limit)->written());
        ClipboardLedger.Host other=failed.open(); check(other.begin(1,new byte[0],65536)==0);
        check(other.poll(1)==ClipboardLedger.UNAVAILABLE); check(other.stats()==0);
        check(other.begin(2,null,0)==ClipboardLedger.INVALID_LIMITS);
        check(other.begin(2,new byte[65537],65536)==ClipboardLedger.TOO_LONG);
    }
    public static void main(String[] args) throws Exception {
        budgets(); cancellation(); runningCancellation(); lifecycleAndBodies(); closeAndPostFailure();
        System.out.println("Android clipboard: UTF-8 budgets, lazy/physical admission, cancellation, receipt identity, lifecycle and exhaustion passed");
    }
}
