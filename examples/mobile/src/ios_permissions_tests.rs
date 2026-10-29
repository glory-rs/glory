use super::*;
use std::cell::RefCell;

#[derive(Default)]
struct State {
    open: i64,
    begin: i64,
    reply: i64,
    stats: i64,
    calls: Vec<Vec<i64>>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn host() -> Host {
    STATE.with_borrow_mut(|s| {
        *s = State {
            open: 17,
            ..Default::default()
        }
    });
    Host {
        open: || {
            STATE.with_borrow_mut(|s| {
                s.calls.push(vec![0]);
                s.open
            })
        },
        begin: |c, id, k, a| {
            STATE.with_borrow_mut(|s| {
                s.calls.push(vec![1, c, id, k, a]);
                s.begin
            })
        },
        poll: |c, id| {
            STATE.with_borrow_mut(|s| {
                s.calls.push(vec![2, c, id]);
                s.reply
            })
        },
        cancel: |c, id| STATE.with_borrow_mut(|s| s.calls.push(vec![3, c, id])),
        close: |c| STATE.with_borrow_mut(|s| s.calls.push(vec![4, c])),
        stats: |c| {
            STATE.with_borrow_mut(|s| {
                s.calls.push(vec![5, c]);
                s.stats
            })
        },
    }
}

#[test]
fn connection_validates_host_replies_and_integer_ids_and_closes_exactly_once() {
    let driver = IosPermissions::open(host()).unwrap();
    assert_eq!(
        driver.begin(0, Permission::Camera, Action::Request),
        Err(PermissionError::InvalidReply)
    );
    assert_eq!(driver.poll(u64::MAX), Err(PermissionError::Exhausted));
    driver.cancel(0);
    driver.cancel(u64::MAX);
    assert_eq!(STATE.with_borrow(|s| s.calls.len()), 1);
    driver
        .begin(i64::MAX as u64, Permission::Notifications, Action::Request)
        .unwrap();
    assert_eq!(
        STATE.with_borrow(|s| s.calls.last().cloned()),
        Some(vec![1, 17, i64::MAX, 6, 1])
    );
    assert_eq!(driver.poll(1), Ok(None));
    STATE.with_borrow_mut(|s| s.reply = 0x105);
    assert_eq!(
        driver.poll(1).unwrap().unwrap().status,
        glory::apace::permissions::Status::Limited
    );
    STATE.with_borrow_mut(|s| {
        s.reply = 0x108;
        s.begin = -4;
        s.stats = 33;
    });
    assert_eq!(driver.poll(1), Err(PermissionError::InvalidReply));
    assert_eq!(
        driver.begin(2, Permission::Camera, Action::Query),
        Err(PermissionError::MissingDeclaration)
    );
    assert_eq!(driver.stats(), Err(PermissionError::InvalidReply));
    driver.cancel(2);
    driver.close();
    driver.close();
    assert_eq!(driver.stats(), Err(PermissionError::Closed));
    assert_eq!(driver.poll(1), Err(PermissionError::Closed));
    assert_eq!(
        driver.begin(1, Permission::Camera, Action::Query),
        Err(PermissionError::Closed)
    );
    driver.cancel(1);
    drop(driver);
    assert_eq!(
        STATE.with_borrow(|s| s.calls.iter().filter(|c| c[0] == 4).count()),
        1
    );
    assert_eq!(
        STATE.with_borrow(|s| s.calls.last().cloned()),
        Some(vec![4, 17])
    );
}

#[test]
fn failed_open_and_invalid_service_limits_do_not_leak_or_reopen_clients() {
    for (code, error) in [
        (-2, PermissionError::Busy),
        (0, PermissionError::InvalidReply),
    ] {
        let host = host();
        STATE.with_borrow_mut(|s| s.open = code);
        assert_eq!(IosPermissions::open(host).err(), Some(error));
        assert_eq!(STATE.with_borrow(|s| s.calls.clone()), vec![vec![0]]);
    }
    let driver = IosPermissions::open(host()).unwrap();
    assert_eq!(
        Permissions::new(
            driver,
            PermissionLimits {
                pending: 0,
                ..Default::default()
            }
        )
        .err(),
        Some(PermissionError::InvalidLimits)
    );
    assert_eq!(
        STATE.with_borrow(|s| s.calls.clone()),
        vec![vec![0], vec![4, 17]]
    );
}
