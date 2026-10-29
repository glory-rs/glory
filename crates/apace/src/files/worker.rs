//! A bounded queue of admitted I/O work. Active work is drained on shutdown;
//! OS calls own their physical permits independently of receiver futures.
use super::*;
use tokio::sync::mpsc;
pub(super) type Work = Pin<Box<dyn Future<Output = ()> + Send>>;
pub(super) struct Worker {
    sender: mpsc::Sender<Work>,
}
impl Worker {
    pub fn new() -> Result<Self, FileRejection> {
        let (sender, mut receiver) = mpsc::channel::<Work>(8);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .map_err(|_| FileRejection::Unavailable)?;
        std::thread::Builder::new()
            .name("glory-files".into())
            .spawn(move || {
                runtime.block_on(async move {
                    let mut tasks = tokio::task::JoinSet::new();
                    loop {
                        tokio::select! {
                            work = receiver.recv(), if tasks.len() < 8 => match work {
                                Some(work) => { tasks.spawn(work); }
                                None => break,
                            },
                            _ = tasks.join_next(), if !tasks.is_empty() => {}
                        }
                    }
                    while tasks.join_next().await.is_some() {}
                })
            })
            .map_err(|_| FileRejection::Unavailable)?;
        Ok(Self { sender })
    }
    pub fn send(&self, work: Work) -> Result<(), FileRejection> {
        self.sender.try_send(work).map_err(|_| FileRejection::Busy)
    }
}
