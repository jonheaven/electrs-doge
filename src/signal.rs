use crossbeam_channel as channel;
use crossbeam_channel::RecvTimeoutError;
use std::thread;
use std::time::{Duration, Instant};

use signal_hook::consts::{SIGINT, SIGTERM};
#[cfg(unix)]
use signal_hook::consts::SIGUSR1;

use crate::errors::*;

#[derive(Clone)] // so multiple threads could wait on signals
pub struct Waiter {
    receiver: channel::Receiver<i32>,
}

#[cfg(unix)]
fn notify(signals: &[i32]) -> channel::Receiver<i32> {
    let (s, r) = channel::bounded(1);
    let mut signals =
        signal_hook::iterator::Signals::new(signals).expect("failed to register signal hook");
    thread::spawn(move || {
        for signal in signals.forever() {
            s.send(signal)
                .unwrap_or_else(|_| panic!("failed to send signal {}", signal));
        }
    });
    r
}

#[cfg(not(unix))]
fn notify(signals: &[i32]) -> channel::Receiver<i32> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let (s, r) = channel::bounded(signals.len());
    let term = Arc::new(AtomicUsize::new(0));

    for &sig in signals {
        signal_hook::flag::register_usize(sig, Arc::clone(&term), sig as usize)
            .unwrap_or_else(|_| panic!("failed to register signal hook for {}", sig));
    }

    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(50));
        let sig = term.swap(0, Ordering::Relaxed);
        if sig != 0 {
            let _ = s.send(sig as i32);
        }
    });

    r
}

impl Waiter {
    pub fn start() -> Waiter {
        #[cfg(unix)]
        let signals = &[SIGINT, SIGTERM, SIGUSR1];
        #[cfg(not(unix))]
        let signals = &[SIGINT, SIGTERM];
        Waiter {
            receiver: notify(signals),
        }
    }

    pub fn wait(&self, duration: Duration, accept_sigusr: bool) -> Result<()> {
        // Determine the deadline time based on the duration, so that it doesn't
        // get pushed back when wait_deadline() recurses
        self.wait_deadline(Instant::now() + duration, accept_sigusr)
    }

    fn wait_deadline(&self, deadline: Instant, accept_sigusr: bool) -> Result<()> {
        match self.receiver.recv_deadline(deadline) {
            #[cfg(unix)]
            Ok(sig) if sig == SIGUSR1 => {
                trace!("notified via SIGUSR1");
                if accept_sigusr {
                    Ok(())
                } else {
                    self.wait_deadline(deadline, accept_sigusr)
                }
            }
            Ok(sig) => bail!(ErrorKind::Interrupt(sig)),
            Err(RecvTimeoutError::Timeout) => Ok(()),
            Err(RecvTimeoutError::Disconnected) => bail!("signal hook channel disconnected"),
        }
    }
}
