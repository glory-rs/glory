//! Application-owned UIKit permission connection. No native object or pointer
//! crosses this ABI; the Swift host validates integers and serializes on main.
use glory::blitz::permissions::{
    Action, NativeStats, Permission, PermissionDriver, PermissionError, PermissionLimits,
    PermissionState, Permissions, bridge,
};
use std::cell::Cell;

#[cfg(target_os = "ios")]
// User-authorized application-only exception (2026-10-27). Swift @_cdecl exports
// use Int64 for every argument/result, accept every integer, own all native
// lifetimes and marshal to main. These functions have no caller preconditions.
// No pointer, unsafe call block or runtime-crate exception is introduced.
#[allow(unsafe_code)]
unsafe extern "C" {
    safe fn glory_permissions_open() -> i64;
    safe fn glory_permissions_begin(client: i64, request: i64, kind: i64, action: i64) -> i64;
    safe fn glory_permissions_poll(client: i64, request: i64) -> i64;
    safe fn glory_permissions_cancel(client: i64, request: i64);
    safe fn glory_permissions_close(client: i64);
    safe fn glory_permissions_stats(client: i64) -> i64;
}

#[derive(Clone, Copy)]
struct Host {
    open: fn() -> i64,
    begin: fn(i64, i64, i64, i64) -> i64,
    poll: fn(i64, i64) -> i64,
    cancel: fn(i64, i64),
    close: fn(i64),
    stats: fn(i64) -> i64,
}

struct IosPermissions {
    host: Host,
    client: Cell<Option<i64>>,
}
impl IosPermissions {
    fn open(host: Host) -> Result<Self, PermissionError> {
        let client = (host.open)();
        if client <= 0 {
            return Err(bridge::error(client));
        }
        Ok(Self {
            host,
            client: Cell::new(Some(client)),
        })
    }
    fn client(&self) -> Result<i64, PermissionError> {
        self.client.get().ok_or(PermissionError::Closed)
    }
    fn id(id: u64) -> Result<i64, PermissionError> {
        let id = i64::try_from(id).map_err(|_| PermissionError::Exhausted)?;
        if id == 0 {
            return Err(PermissionError::InvalidReply);
        }
        Ok(id)
    }
}
impl PermissionDriver for IosPermissions {
    fn begin(&self, id: u64, kind: Permission, action: Action) -> Result<(), PermissionError> {
        bridge::accepted((self.host.begin)(
            self.client()?,
            Self::id(id)?,
            kind as i64,
            action as i64,
        ))
    }
    fn poll(&self, id: u64) -> Result<Option<PermissionState>, PermissionError> {
        bridge::reply((self.host.poll)(self.client()?, Self::id(id)?))
    }
    fn cancel(&self, id: u64) {
        if let (Some(client), Ok(id)) = (self.client.get(), Self::id(id)) {
            (self.host.cancel)(client, id);
        }
    }
    fn close(&self) {
        if let Some(client) = self.client.take() {
            (self.host.close)(client);
        }
    }
    fn stats(&self) -> Result<NativeStats, PermissionError> {
        bridge::stats((self.host.stats)(self.client()?))
    }
}
impl Drop for IosPermissions {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(target_os = "ios")]
/// Open an explicit app-owned permission client. Construction does not query or
/// request OS authorization; close it when its requesting Owner retires.
pub fn service() -> Result<Permissions, PermissionError> {
    Permissions::new(
        IosPermissions::open(native_host())?,
        PermissionLimits::default(),
    )
}

#[cfg(target_os = "ios")]
fn native_host() -> Host {
    Host {
        open: || glory_permissions_open(),
        begin: |c, i, k, a| glory_permissions_begin(c, i, k, a),
        poll: |c, i| glory_permissions_poll(c, i),
        cancel: |c, i| glory_permissions_cancel(c, i),
        close: |c| glory_permissions_close(c),
        stats: |c| glory_permissions_stats(c),
    }
}

#[cfg(all(target_os = "ios", feature = "permissions-probe"))]
pub(crate) fn audit_integer_abi() {
    for client in [i64::MIN, -1, 0, i64::MAX] {
        assert_eq!(glory_permissions_begin(client, 1, 1, 0), -3);
        assert_eq!(glory_permissions_poll(client, 1), -3);
        assert_eq!(glory_permissions_stats(client), -3);
        glory_permissions_cancel(client, i64::MIN);
        glory_permissions_close(client);
    }
    let driver = IosPermissions::open(native_host()).unwrap();
    let client = driver.client().unwrap();
    for (id, kind, action) in [
        (0, 1, 0),
        (i64::MIN, 1, 0),
        (1, i64::MAX, 0),
        (1, 1, i64::MIN),
    ] {
        assert_eq!(glory_permissions_begin(client, id, kind, action), -6);
        assert_eq!(driver.stats().unwrap().records, 0);
    }
    driver.close();
    assert_eq!(glory_permissions_stats(client), -3);
    eprintln!("Glory iOS permission integer ABI validation passed");
}

#[cfg(test)]
#[path = "ios_permissions_tests.rs"]
mod tests;
