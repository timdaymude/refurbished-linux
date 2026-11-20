//! Reaping for children that nobody waits for.
//!
//! The shell starts programs and then forgets about them (a browser window, a
//! terminal, `systemctl poweroff`). A `Child` that is dropped without being
//! waited for stays in the process table as a zombie for as long as its parent
//! lives, and the shell lives for the whole session. That is where the zombies
//! in the live system's `ps` output came from: every launched program left one
//! behind. A single background thread now keeps them in a list and calls
//! `try_wait` on each of them, so a long-running browser never blocks the
//! cleanup of a short-lived helper.

use std::process::Child;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

static QUEUE: OnceLock<Mutex<mpsc::Sender<Child>>> = OnceLock::new();

#[cfg(test)]
pub(crate) static REAPED: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

fn sender() -> &'static Mutex<mpsc::Sender<Child>> {
    QUEUE.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Child>();
        std::thread::Builder::new()
            .name("child-reaper".into())
            .spawn(move || run(rx))
            .expect("start the child reaper");
        Mutex::new(tx)
    })
}

/// Hand a child to the reaper. Never blocks and never fails in practice: the
/// queue lives for the whole process.
pub fn adopt(child: Child) {
    if let Ok(tx) = sender().lock() {
        let _ = tx.send(child);
    }
}

fn run(rx: Receiver<Child>) {
    let mut pending: Vec<Child> = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(child) => pending.push(child),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        pending.retain_mut(|child| match child.try_wait() {
            Ok(Some(_)) | Err(_) => {
                #[cfg(test)]
                REAPED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                false
            }
            Ok(None) => true,
        });
    }
    for mut child in pending {
        let _ = child.wait();
    }
}

/// Spawn a detached program and let the reaper clean up after it.
pub fn spawn_detached(
    prog: &str,
    args: &[&str],
    stdin: std::process::Stdio,
    stdout: std::process::Stdio,
    stderr: std::process::Stdio,
) -> std::io::Result<()> {
    let child = std::process::Command::new(prog)
        .args(args)
        .stdin(stdin)
        .stdout(stdout)
        .stderr(stderr)
        .spawn()?;
    adopt(child);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    #[test]
    fn reaps_children_and_does_not_block_on_long_lived_ones() {
        let before = REAPED.load(std::sync::atomic::Ordering::Relaxed);
        // A long-lived child is adopted first: a reaper that waited on each
        // child in turn would never get to the short one after it.
        let long = Command::new("sleep")
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");
        adopt(long);
        for _ in 0..3 {
            let short = Command::new("true")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn true");
            adopt(short);
        }
        let mut waited = 0;
        while waited < 40 {
            std::thread::sleep(Duration::from_millis(100));
            waited += 1;
            if REAPED.load(std::sync::atomic::Ordering::Relaxed) >= before + 3 {
                return;
            }
        }
        panic!("the reaper did not collect the short-lived children");
    }
}
