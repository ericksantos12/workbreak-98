use std::{
    env,
    error::Error,
    sync::mpsc::RecvTimeoutError,
    time::{Duration, Instant, SystemTime},
};

use flowmodoro_lib::{
    clock::unbiased_time_100ns,
    power::{PowerEvent, PowerNotification, PowerSubscription},
};

fn main() -> Result<(), Box<dyn Error>> {
    let run_for = parse_duration()?;
    let (subscription, events) = PowerSubscription::register()?;
    let started = Instant::now();
    let wall_start = SystemTime::now();
    let unbiased_start = unbiased_time_100ns()?;

    println!("probe_ready: press Ctrl+C after testing suspend/resume");
    loop {
        match events.recv_timeout(Duration::from_secs(1)) {
            Ok(PowerNotification::Event(PowerEvent::Suspending { at_100ns })) => {
                println!("power_event: suspending at_100ns={at_100ns}");
            }
            Ok(PowerNotification::Event(PowerEvent::Resumed)) => {
                println!("power_event: resumed");
            }
            Ok(PowerNotification::ClockError(error)) => {
                eprintln!("clock_error: {error}");
            }
            Err(RecvTimeoutError::Timeout) => {
                let wall_ms = SystemTime::now()
                    .duration_since(wall_start)
                    .map(|duration| duration.as_millis())
                    .unwrap_or_default();
                let unbiased_ms = (unbiased_time_100ns()? - unbiased_start) / 10_000;
                println!("sample: wall_ms={wall_ms} unbiased_ms={unbiased_ms}");
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err("power notification channel disconnected".into());
            }
        }

        if run_for.is_some_and(|duration| started.elapsed() >= duration) {
            break;
        }
    }

    drop(subscription);
    println!("probe_stopped");
    Ok(())
}

fn parse_duration() -> Result<Option<Duration>, Box<dyn Error>> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        None => Ok(None),
        Some("--seconds") => {
            let seconds: u64 = args.next().ok_or("missing value for --seconds")?.parse()?;
            if args.next().is_some() {
                return Err("unexpected extra arguments".into());
            }
            Ok(Some(Duration::from_secs(seconds)))
        }
        Some(argument) => Err(format!("unknown argument: {argument}").into()),
    }
}
