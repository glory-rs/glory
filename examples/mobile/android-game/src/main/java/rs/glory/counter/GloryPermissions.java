package rs.glory.counter;

import android.Manifest;
import android.app.Activity;
import android.app.NotificationManager;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import java.lang.ref.WeakReference;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;

// Application-owned scalar bridge. All platform queries/requests run on the
// Java main thread. No JNI callbacks or strong Activity references cross Rust.
public final class GloryPermissions {
    private static final long READY = 0x100, GRANTED = 2, DENIED = 3, LIMITED = 5;
    private static final long UNSUPPORTED = -1, BUSY = -2, CLOSED = -3,
        MISSING = -4, UNAVAILABLE = -5, BACKEND = -6, EXHAUSTED = -7;

    private static final class Record {
        final long id, kind, action;
        boolean dispatched;
        long reply;
        Record(long id, long kind, long action) { this.id = id; this.kind = kind; this.action = action; }
    }
    private static final class Ticket {
        final WeakReference<Host> host;
        final Record record;
        final String[] permissions;
        final int code;
        Ticket(Host host, Record record, String[] permissions, int code) {
            this.host = new WeakReference<>(host); this.record = record;
            this.permissions = permissions; this.code = code;
        }
    }
    private static final class Work {
        final Host host; final Record record;
        Work(Host host, Record record) { this.host = host; this.record = record; }
    }

    public static final class Coordinator {
        final Object lock = new Object();
        final WeakReference<Activity> activity;
        final Handler main = new Handler(Looper.getMainLooper());
        final ArrayList<WeakReference<Host>> hosts = new ArrayList<>();
        boolean closed, scheduled, resumed;
        int nextCode = 0x6500;
        Ticket active;
        public Coordinator(Activity activity) { this.activity = new WeakReference<>(activity); }
        public void resumed(boolean value) { synchronized (lock) { resumed = value; } }
        public Host open() {
            synchronized (lock) {
                hosts.removeIf(ref -> { Host host = ref.get(); return host == null || host.closed; });
                if (closed || hosts.size() >= 32) return null;
                Host host = new Host(this); hosts.add(new WeakReference<>(host)); return host;
            }
        }
        private void schedule() {
            if (!scheduled && !closed) {
                scheduled = true;
                if (!main.post(this::drain)) close();
            }
        }
        private void drain() {
            ArrayList<Work> work = new ArrayList<>();
            synchronized (lock) {
                scheduled = false;
                if (closed) return;
                hosts.removeIf(ref -> { Host host = ref.get(); return host == null || host.closed; });
                batch: for (WeakReference<Host> ref : hosts) {
                    Host host = ref.get();
                    if (host != null) for (Record record : host.records.values()) {
                        if (!record.dispatched) {
                            record.dispatched = true; work.add(new Work(host, record));
                            if (work.size() == 16) break batch;
                        }
                    }
                }
            }
            for (Work item : work) run(item.host, item.record);
            synchronized (lock) {
                boolean more = false;
                for (WeakReference<Host> ref : hosts) {
                    Host host = ref.get();
                    if (host != null) for (Record record : host.records.values()) more |= !record.dispatched;
                }
                if (more) schedule();
            }
        }
        private void finish(Host host, Record record, long reply) {
            synchronized (lock) {
                if (!closed && !host.closed && host.records.get(record.id) == record) record.reply = reply;
            }
        }
        private void run(Host host, Record record) {
            Activity current = activity.get();
            synchronized (lock) {
                if (closed || host.closed || host.records.get(record.id) != record) return;
            }
            if (current == null || current.isDestroyed() || current.isFinishing()) { finish(host, record, UNAVAILABLE); return; }
            try {
                String[] required = required(record.kind);
                if (required == null) { finish(host, record, UNSUPPORTED); return; }
                if (record.action == 1 && required.length != 0) {
                    PackageInfo info = current.getPackageManager().getPackageInfo(current.getPackageName(), PackageManager.GET_PERMISSIONS);
                    java.util.List<String> declared = info.requestedPermissions == null ? java.util.Collections.emptyList() : Arrays.asList(info.requestedPermissions);
                    if (!declared.containsAll(Arrays.asList(required))) { finish(host, record, MISSING); return; }
                }
                long state = state(current, record.kind, required);
                if (record.action == 0 || (state & 7) == GRANTED || required.length == 0) { finish(host, record, state); return; }
                Ticket ticket;
                synchronized (lock) {
                    if (closed || host.closed || host.records.get(record.id) != record) return;
                    if (active != null) { finish(host, record, BUSY); return; }
                    if (!resumed) { finish(host, record, UNAVAILABLE); return; }
                    if (nextCode > 0x7fff) { finish(host, record, EXHAUSTED); return; }
                    ticket = new Ticket(host, record, required, nextCode++);
                    active = ticket;
                }
                // Once admitted, cancellation withdraws delivery but cannot
                // dismiss the OS dialog or refund this slot before its callback.
                try { current.requestPermissions(required, ticket.code); }
                catch (RuntimeException error) {
                    synchronized (lock) { if (active == ticket) active = null; }
                    finish(host, record, BACKEND);
                }
            } catch (PackageManager.NameNotFoundException | RuntimeException error) { finish(host, record, BACKEND); }
        }
        public boolean result(int code, String[] permissions, int[] grants) {
            Ticket ticket;
            synchronized (lock) {
                ticket = active;
                if (closed || ticket == null || ticket.code != code) return false;
                if (permissions.length != 0 && !Arrays.equals(ticket.permissions, permissions)) return false;
                active = null;
            }
            Host host = ticket.host.get(); Activity current = activity.get();
            if (host != null) {
                long reply = UNAVAILABLE;
                if (permissions.length != 0 && grants.length == permissions.length && current != null) {
                    try { reply = state(current, ticket.record.kind, ticket.permissions); }
                    catch (RuntimeException error) { reply = BACKEND; }
                }
                finish(host, ticket.record, reply);
            }
            return true;
        }
        public void close() {
            synchronized (lock) {
                closed = true; active = null; activity.clear();
                for (WeakReference<Host> ref : hosts) {
                    Host host = ref.get(); if (host != null) { host.closed = true; host.records.clear(); }
                }
                hosts.clear();
            }
        }
    }

    public static final class Host {
        final Coordinator coordinator;
        final LinkedHashMap<Long, Record> records = new LinkedHashMap<>();
        boolean closed;
        Host(Coordinator coordinator) { this.coordinator = coordinator; }
        public long protocolVersion() { return 1; }
        public long begin(long id, long kind, long action) {
            synchronized (coordinator.lock) {
                if (closed || coordinator.closed) return CLOSED;
                if (id <= 0 || kind < 1 || kind > 6 || action < 0 || action > 1 || records.containsKey(id)) return BACKEND;
                if (records.size() >= 32) return BUSY;
                records.put(id, new Record(id, kind, action)); coordinator.schedule();
                return closed ? CLOSED : 0;
            }
        }
        public long poll(long id) {
            synchronized (coordinator.lock) {
                if (closed || coordinator.closed) return CLOSED;
                Record record = records.get(id); if (record == null) return CLOSED;
                if (record.reply != 0) records.remove(id);
                return record.reply;
            }
        }
        public void cancel(long id) { synchronized (coordinator.lock) { records.remove(id); } }
        public void close() {
            synchronized (coordinator.lock) {
                if (closed) return;
                closed = true; records.clear();
                coordinator.hosts.removeIf(ref -> ref.get() == null || ref.get() == this);
            }
        }
        public long stats() {
            synchronized (coordinator.lock) {
                if (closed || coordinator.closed) return CLOSED;
                return records.size() | (coordinator.active == null ? 0 : 0x10000);
            }
        }
    }

    private static String[] required(long kind) {
        switch ((int)kind) {
            case 1: return new String[]{Manifest.permission.CAMERA};
            case 2: return new String[]{Manifest.permission.RECORD_AUDIO};
            case 3:
                if (Build.VERSION.SDK_INT >= 34) return new String[]{Manifest.permission.READ_MEDIA_IMAGES, Manifest.permission.READ_MEDIA_VIDEO, Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED};
                if (Build.VERSION.SDK_INT >= 33) return new String[]{Manifest.permission.READ_MEDIA_IMAGES, Manifest.permission.READ_MEDIA_VIDEO};
                return new String[]{Manifest.permission.READ_EXTERNAL_STORAGE};
            case 4: return Build.VERSION.SDK_INT >= 29 ? new String[0] : new String[]{Manifest.permission.WRITE_EXTERNAL_STORAGE};
            case 5: return new String[]{Manifest.permission.ACCESS_COARSE_LOCATION, Manifest.permission.ACCESS_FINE_LOCATION};
            case 6: return Build.VERSION.SDK_INT >= 33 ? new String[]{Manifest.permission.POST_NOTIFICATIONS} : new String[0];
            default: return null;
        }
    }
    private static boolean granted(Activity activity, String permission) { return activity.checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED; }
    // PhotosRead covers both media kinds. Selected access is an alternative,
    // never an extra prerequisite for already-granted full library access.
    static long photosStatus(boolean images, boolean videos, boolean selected) {
        return images && videos ? GRANTED : images || videos || selected ? LIMITED : DENIED;
    }
    private static long state(Activity activity, long kind, String[] required) {
        long status = GRANTED; boolean rationale = false;
        if (kind == 6 && !activity.getSystemService(NotificationManager.class).areNotificationsEnabled()) status = DENIED;
        for (String permission : required) {
            if (kind == 3 && Build.VERSION.SDK_INT >= 34 && permission.equals(Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED)) continue;
            if (!granted(activity, permission)) { status = DENIED; rationale |= activity.shouldShowRequestPermissionRationale(permission); }
        }
        if (kind == 3 && Build.VERSION.SDK_INT >= 33) {
            status = photosStatus(granted(activity, Manifest.permission.READ_MEDIA_IMAGES), granted(activity, Manifest.permission.READ_MEDIA_VIDEO),
                Build.VERSION.SDK_INT >= 34 && granted(activity, Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED));
        }
        if (kind == 5 && status == DENIED && granted(activity, Manifest.permission.ACCESS_COARSE_LOCATION)) status = LIMITED;
        return READY | status | (rationale ? 0x20 : 0);
    }
}
