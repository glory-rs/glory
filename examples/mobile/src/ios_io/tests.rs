use super::*;
use std::{sync::LazyLock, task::Waker};
#[derive(Default)]
struct State {
    calls: Vec<(i64, i64, i64, i64)>,
    body: Vec<u8>,
    code: i64,
    length: Option<i64>,
    uploaded: Vec<u8>,
}
static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State::default()));
static SERIAL: Mutex<()> = Mutex::new(());
fn host(_: i64, _: i64, op: i64, id: i64, a: i64, b: i64) -> i64 {
    let mut state = STATE.lock().unwrap();
    state.calls.push((op, id, a, b));
    match op {
        12 => 1,
        0 => 31,
        5 => state.code,
        6 => state.length.unwrap_or(state.body.len() as i64),
        7 => {
            let mut bytes = [0; 8];
            let chunk = &state.body[a as usize..];
            let n = chunk.len().min(8);
            bytes[..n].copy_from_slice(&chunk[..n]);
            i64::from_le_bytes(bytes)
        }
        3 => {
            state.uploaded.extend(b.to_le_bytes());
            0
        }
        _ => 0,
    }
}
fn context() -> Context<'static> {
    Context::from_waker(Waker::noop())
}
#[test]
fn native_jobs_are_lazy_cancel_exact_receipts_keep_client_alive_and_bound_reply_before_word_reads()
{
    let _serial = SERIAL.lock().unwrap();
    *STATE.lock().unwrap() = State::default();
    let client = Client::open(host, 1).unwrap();
    let calls = STATE.lock().unwrap().calls.len();
    drop(client.job(0, vec![1, 2, 3], 0));
    assert_eq!(STATE.lock().unwrap().calls.len(), calls);
    let mut pending = Box::pin(client.job(1, vec![0, 1, 2, 3, 4, 5, 6, 7, 8], 16));
    assert!(pending.as_mut().poll(&mut context()).is_pending());
    assert_eq!(
        STATE.lock().unwrap().uploaded[..9],
        [0, 1, 2, 3, 4, 5, 6, 7, 8]
    );
    drop(client);
    assert!(!STATE.lock().unwrap().calls.iter().any(|v| v.0 == 1));
    drop(pending);
    assert_eq!(
        STATE
            .lock()
            .unwrap()
            .calls
            .iter()
            .filter(|v| v.0 == 8)
            .count(),
        1
    );
    assert_eq!(
        STATE
            .lock()
            .unwrap()
            .calls
            .iter()
            .filter(|v| v.0 == 1)
            .count(),
        1
    );
    for length in [-1, 17] {
        *STATE.lock().unwrap() = State {
            code: 2,
            length: Some(length),
            ..Default::default()
        };
        let client = Client::open(host, 1).unwrap();
        let mut job = Box::pin(client.job(1, vec![], 16));
        assert!(matches!(
            job.as_mut().poll(&mut context()),
            Poll::Ready(Err(FAILED))
        ));
        assert!(!STATE.lock().unwrap().calls.iter().any(|v| v.0 == 7));
        drop(job);
        drop(client);
        assert_eq!(
            STATE
                .lock()
                .unwrap()
                .calls
                .iter()
                .filter(|v| v.0 == 8)
                .count(),
            1
        );
    }
}
#[cfg(feature = "clipboard")]
#[test]
fn clipboard_checks_raw_and_normalized_bounds_and_releases_terminal_receipts() {
    use glory::blitz::clipboard::{ClipboardDriver, ClipboardError};
    let _serial = SERIAL.lock().unwrap();
    *STATE.lock().unwrap() = State {
        code: 1,
        ..Default::default()
    };
    let driver = IosClipboard::open(host).unwrap();
    for text in ["x".repeat(65537), "\0".repeat(65536)] {
        assert!(matches!(
            driver.write(text).as_mut().poll(&mut context()),
            Poll::Ready(Err(ClipboardError::TooLong))
        ));
    }
    assert!(!STATE.lock().unwrap().calls.iter().any(|v| v.0 == 2));
    let mut written = driver.write("中😀\r\n\0".into());
    assert!(matches!(
        written.as_mut().poll(&mut context()),
        Poll::Ready(Ok(()))
    ));
    let expected = "中😀\n�".as_bytes();
    assert_eq!(&STATE.lock().unwrap().uploaded[..expected.len()], expected);
    // Keeping a completed future must not retain its native receipt.
    assert_eq!(
        STATE
            .lock()
            .unwrap()
            .calls
            .iter()
            .filter(|v| v.0 == 8)
            .count(),
        1
    );
    {
        let mut s = STATE.lock().unwrap();
        s.code = 2;
        s.body = vec![0xff];
    }
    assert!(matches!(
        driver.read(16).as_mut().poll(&mut context()),
        Poll::Ready(Err(ClipboardError::Failed))
    ));
    driver.close();
    driver.close();
    drop(written);
    drop(driver);
    assert_eq!(
        STATE
            .lock()
            .unwrap()
            .calls
            .iter()
            .filter(|v| v.0 == 1)
            .count(),
        1
    );
}
#[test]
fn parallel_submissions_keep_monotonic_begin_and_commit_order() {
    let _serial = SERIAL.lock().unwrap();
    *STATE.lock().unwrap() = State {
        code: 1,
        ..Default::default()
    };
    let client = Client::open(host, 2).unwrap();
    std::thread::scope(|scope| {
        for _ in 0..16 {
            let client = client.clone();
            scope.spawn(move || {
                let mut job = Box::pin(client.job(0, vec![1; 24], 0));
                assert!(matches!(
                    job.as_mut().poll(&mut context()),
                    Poll::Ready(Ok(_))
                ));
            });
        }
    });
    let state = STATE.lock().unwrap();
    let begins: Vec<_> = state
        .calls
        .iter()
        .filter(|v| v.0 == 2)
        .map(|v| v.1)
        .collect();
    let commits: Vec<_> = state
        .calls
        .iter()
        .filter(|v| v.0 == 4)
        .map(|v| v.1)
        .collect();
    assert_eq!(begins, (1..=16).collect::<Vec<_>>());
    assert_eq!(begins, commits);
}
#[cfg(feature = "files")]
#[test]
fn file_adoption_and_reader_release_are_scoped_to_native_ids() {
    use glory::blitz::files::FilePicker;
    use glory::render::FileConfig;
    let _serial = SERIAL.lock().unwrap();
    let mut body = 1u32.to_be_bytes().to_vec();
    body.extend(77u64.to_be_bytes());
    body.extend(9u64.to_be_bytes());
    body.extend(5u16.to_be_bytes());
    body.extend(10u16.to_be_bytes());
    body.extend(b"a.txttext/plain");
    *STATE.lock().unwrap() = State {
        code: 2,
        body,
        ..Default::default()
    };
    let driver = IosFiles::open(host).unwrap();
    let mut future = driver.pick(FileConfig::default());
    let files = match future.as_mut().poll(&mut context()) {
        Poll::Ready(Ok(Some(files))) => files,
        _ => panic!("native metadata reply"),
    };
    assert_eq!(files[0].name, "a.txt");
    assert_eq!(
        STATE
            .lock()
            .unwrap()
            .calls
            .iter()
            .filter(|v| v.0 == 9)
            .count(),
        1
    );
    assert!(!STATE.lock().unwrap().calls.iter().any(|v| v.0 == 8));
    drop(future);
    drop(driver);
    assert!(!STATE.lock().unwrap().calls.iter().any(|v| v.0 == 1));
    assert!(matches!(
        files[0].reader.read(8, 2).as_mut().poll(&mut context()),
        Poll::Ready(Err(_))
    ));
    drop(files);
    assert!(
        STATE
            .lock()
            .unwrap()
            .calls
            .iter()
            .any(|v| v.0 == 10 && v.1 == 77)
    );
    assert_eq!(
        STATE
            .lock()
            .unwrap()
            .calls
            .iter()
            .filter(|v| v.0 == 1)
            .count(),
        1
    );
}

#[cfg(feature = "files")]
#[test]
fn unpolled_read_retains_selected_capability_and_terminal_future_releases_it() {
    use glory::blitz::files::FilePicker;
    use glory::render::FileConfig;
    let _serial = SERIAL.lock().unwrap();
    let mut body = 1u32.to_be_bytes().to_vec();
    body.extend(77u64.to_be_bytes());
    body.extend(9u64.to_be_bytes());
    body.extend(5u16.to_be_bytes());
    body.extend(10u16.to_be_bytes());
    body.extend(b"a.txttext/plain");
    *STATE.lock().unwrap() = State {
        code: 2,
        body,
        ..Default::default()
    };
    let driver = IosFiles::open(host).unwrap();
    let mut pick = driver.pick(FileConfig::default());
    let files = match pick.as_mut().poll(&mut context()) {
        Poll::Ready(Ok(Some(files))) => files,
        _ => panic!("selected native file"),
    };
    let mut read = files[0].reader.read(0, 9);
    drop(files);
    drop(pick);
    drop(driver);
    assert!(!STATE.lock().unwrap().calls.iter().any(|v| v.0 == 10));
    {
        let mut state = STATE.lock().unwrap();
        state.calls.clear();
        state.body = b"original!".to_vec();
    }
    assert!(
        matches!(read.as_mut().poll(&mut context()),Poll::Ready(Ok(bytes)) if bytes==b"original!")
    );
    assert!(
        STATE
            .lock()
            .unwrap()
            .calls
            .iter()
            .any(|v| v.0 == 10 && v.1 == 77)
    );
    assert_eq!(
        STATE
            .lock()
            .unwrap()
            .calls
            .iter()
            .filter(|v| v.0 == 1)
            .count(),
        1
    );
    // Keep the completed Future alive while checking all foreign leases drained.
    drop(read);
    assert_eq!(
        STATE
            .lock()
            .unwrap()
            .calls
            .iter()
            .filter(|v| v.0 == 10)
            .count(),
        1
    );
}
