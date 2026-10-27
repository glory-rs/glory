use super::*;
use glory::blitz::clipboard::{ClipboardDriver, ClipboardError, Reply};
use glory::render::{MAX_CLIPBOARD_BYTES, normalize_text};
#[derive(Clone)]
pub struct IosClipboard(Arc<Client>);
fn error(code: i64) -> ClipboardError {
    match code {
        -1 => ClipboardError::Unavailable,
        -2 => ClipboardError::Busy,
        -6 => ClipboardError::TooLong,
        -8 => ClipboardError::Denied,
        _ => ClipboardError::Failed,
    }
}
impl IosClipboard {
    #[cfg(feature = "ios-io-probe")]
    pub(super) fn probe(&self, action: i64) -> i64 {
        (self.0.host)(1, self.0.id, 100, 0, action, 0)
    }
    pub(super) fn open(host: Host) -> Result<Self, ClipboardError> {
        Client::open(host, 1).map(Self).map_err(error)
    }
    pub fn close(&self) {
        self.0.close();
    }
    /// Current receipt records and actual queued/running native operations.
    pub fn stats(&self) -> Result<(usize, usize), ClipboardError> {
        let records = self.0.call(11, 0, 0, 0);
        let running = self.0.call(11, 0, 1, 0);
        if records < 0 {
            return Err(error(records));
        }
        if running < 0 {
            return Err(error(running));
        }
        if records > 1 || running > 1 {
            return Err(ClipboardError::Failed);
        }
        Ok((records as usize, running as usize))
    }
}
impl ClipboardDriver for IosClipboard {
    fn write(&self, text: String) -> Reply<()> {
        if text.len() > MAX_CLIPBOARD_BYTES {
            return Box::pin(std::future::ready(Err(ClipboardError::TooLong)));
        }
        let text = normalize_text(&text, true);
        if text.len() > MAX_CLIPBOARD_BYTES {
            return Box::pin(std::future::ready(Err(ClipboardError::TooLong)));
        }
        let job = self.0.job(0, text.as_bytes().to_vec(), 0);
        Box::pin(async move {
            let reply = job.await.map_err(error)?;
            if reply.code != 1 {
                return Err(ClipboardError::Failed);
            }
            Ok(())
        })
    }
    fn read(&self, max_bytes: usize) -> Reply<String> {
        if max_bytes == 0 || max_bytes > MAX_CLIPBOARD_BYTES {
            return Box::pin(std::future::ready(Err(ClipboardError::InvalidLimits)));
        }
        let job = self
            .0
            .job(1, (max_bytes as u32).to_be_bytes().to_vec(), max_bytes);
        Box::pin(async move {
            let mut reply = job.await.map_err(error)?;
            if reply.code != 2 {
                return Err(ClipboardError::Failed);
            }
            String::from_utf8(std::mem::take(&mut reply.bytes)).map_err(|_| ClipboardError::Failed)
        })
    }
}
