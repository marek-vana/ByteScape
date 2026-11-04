//! Explicit local validation commands; every measurement uses the public scan service.
use crate::scan::{Request, Scan, State};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub fn peak_rss_kib() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: getrusage initializes the pointed-to rusage on success.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return None;
    }
    let rss = unsafe { usage.assume_init() }.ru_maxrss as u64;
    Some(if cfg!(target_os = "macos") {
        rss / 1024
    } else {
        rss
    })
}

pub fn summary(samples: &[f64]) -> (f64, f64, f64) {
    if samples.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    (
        sorted.iter().sum::<f64>() / sorted.len() as f64,
        sorted[((sorted.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)],
        *sorted.last().unwrap(),
    )
}

pub fn scan_benchmark(args: &[std::ffi::OsString]) -> Result<(), String> {
    let root = args
        .get(2)
        .map(PathBuf::from)
        .ok_or("--scan-benchmark requires a root path")?;
    let number = |name: &str| -> Result<Option<u64>, String> {
        args.iter()
            .position(|a| a == name)
            .map(|i| {
                args.get(i + 1)
                    .and_then(|a| a.to_str())
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!("{name} requires a nonnegative integer"))
            })
            .transpose()
    };
    let poll_interval = Duration::from_millis(number("--poll-ms")?.unwrap_or(0));
    let cancel_after = number("--cancel-after-ms")?.map(Duration::from_millis);
    let started = Instant::now();
    let scan = Scan::start(Request::new(root, 808));
    let mut next_poll = started;
    let mut cancelled = None;
    let mut events = 0;
    let mut worker_finished_ms = None;
    let mut first_result_ms = None;
    let terminal = loop {
        if worker_finished_ms.is_none() && scan.worker_finished() {
            worker_finished_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
        }
        if started.elapsed() > Duration::from_secs(300) {
            scan.cancel();
            return Err("scan benchmark timed out".into());
        }
        if cancelled.is_none() && cancel_after.is_some_and(|after| started.elapsed() >= after) {
            let at = Instant::now();
            scan.cancel();
            cancelled = Some(at);
            // Cancellation latency is observed immediately, independently of slow progress polling.
            next_poll = Instant::now();
        }
        if Instant::now() >= next_poll {
            if let Ok(event) = scan.poll() {
                events += 1;
                if event.data.is_some() && first_result_ms.is_none() {
                    first_result_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
                }
                if event.state != State::Running {
                    break event;
                }
            }
            next_poll = Instant::now() + poll_interval;
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let cancel_ms = cancelled.map(|at| at.elapsed().as_secs_f64() * 1000.0);
    let data = terminal.data.as_ref();
    let cleanup = Instant::now();
    while !scan.worker_finished() {
        if cleanup.elapsed() > Duration::from_secs(30) {
            return Err("worker cleanup timed out after terminal state".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let cleanup_ms = cleanup.elapsed().as_secs_f64() * 1000.0;
    let worker_finished_ms = worker_finished_ms.unwrap_or(started.elapsed().as_secs_f64() * 1000.0);
    println!(
        "state={:?}\nelapsed_ms={elapsed_ms:.3}\nfirst_result_ms={:.3}\nobserved_events={events}\nrevision={}\nfiles={}\nnodes={}\nlogical={}\nallocated={}\nproblems={}\npeak_rss_kib={}\ncancel_observed_ms={}",
        terminal.state,
        first_result_ms.unwrap_or(0.0),
        terminal.revision,
        data.map_or(0, |d| d.files),
        data.map_or(0, |d| d.nodes.len()),
        data.map_or(0, |d| d.nodes[0].logical),
        data.map_or(0, |d| d.nodes[0].allocated),
        terminal.problems.len(),
        peak_rss_kib().unwrap_or(0),
        cancel_ms.map_or_else(|| "not_requested".into(), |v| format!("{v:.3}"))
    );
    println!("worker_finished_observed_ms={worker_finished_ms:.3}");
    println!("cleanup_after_terminal_ms={:.3}", cleanup_ms);
    if scan.poll().is_ok() {
        return Err("more than one terminal event".into());
    }
    if matches!(terminal.state, State::Failed | State::Unavailable) {
        return Err("source scan failed; see terminal state".into());
    }
    Ok(())
}
