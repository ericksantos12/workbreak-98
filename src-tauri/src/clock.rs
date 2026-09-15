use std::io;

use windows_sys::Win32::System::WindowsProgramming::QueryUnbiasedInterruptTime;

pub trait Clock: Send + Sync + 'static {
    fn now_100ns(&self) -> io::Result<u64>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_100ns(&self) -> io::Result<u64> {
        unbiased_time_100ns()
    }
}

pub fn unbiased_time_100ns() -> io::Result<u64> {
    let mut value = 0;
    // SAFETY: Windows writes one u64 to the valid out pointer for this call.
    let succeeded = unsafe { QueryUnbiasedInterruptTime(&mut value) };

    if succeeded == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::unbiased_time_100ns;

    #[test]
    fn unbiased_time_is_monotonic_while_awake() {
        let first = unbiased_time_100ns().expect("Windows clock should be available");
        let second = unbiased_time_100ns().expect("Windows clock should remain available");

        assert!(second >= first);
    }
}
