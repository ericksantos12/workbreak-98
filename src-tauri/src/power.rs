use std::{
    ffi::c_void,
    io, ptr,
    sync::mpsc::{self, Receiver, Sender},
};

use windows_sys::Win32::{
    Foundation::{ERROR_INVALID_PARAMETER, ERROR_SUCCESS, HANDLE},
    System::Power::{
        PowerRegisterSuspendResumeNotification, PowerUnregisterSuspendResumeNotification,
        DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS,
    },
    UI::WindowsAndMessaging::{
        DEVICE_NOTIFY_CALLBACK, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMECRITICAL,
        PBT_APMRESUMESTANDBY, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND,
    },
};

use crate::clock::unbiased_time_100ns;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerEvent {
    Suspending { at_100ns: u64 },
    Resumed,
}

#[derive(Debug)]
pub enum PowerNotification {
    Event(PowerEvent),
    ClockError(io::Error),
}

struct CallbackContext {
    sender: Sender<PowerNotification>,
}

pub struct PowerSubscription {
    handle: *mut c_void,
    _context: Box<CallbackContext>,
}

// The registration owns a stable boxed callback context and is dropped by one worker thread.
unsafe impl Send for PowerSubscription {}

impl PowerSubscription {
    pub fn register() -> io::Result<(Self, Receiver<PowerNotification>)> {
        let (sender, receiver) = mpsc::channel();
        let mut context = Box::new(CallbackContext { sender });
        let mut parameters = DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
            Callback: Some(power_callback),
            Context: (&mut *context as *mut CallbackContext).cast(),
        };
        let mut handle = ptr::null_mut();

        // SAFETY: parameters and its boxed context remain valid for the registration lifetime.
        let status = unsafe {
            PowerRegisterSuspendResumeNotification(
                DEVICE_NOTIFY_CALLBACK,
                (&mut parameters as *mut DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS).cast::<c_void>()
                    as HANDLE,
                &mut handle,
            )
        };
        win32_result(status)?;

        Ok((
            Self {
                handle,
                _context: context,
            },
            receiver,
        ))
    }

    fn unregister(&mut self) -> io::Result<()> {
        if self.handle.is_null() {
            return Ok(());
        }

        // SAFETY: handle was returned by PowerRegisterSuspendResumeNotification.
        let status = unsafe { PowerUnregisterSuspendResumeNotification(self.handle as isize) };
        self.handle = ptr::null_mut();
        win32_result(status)
    }
}

impl Drop for PowerSubscription {
    fn drop(&mut self) {
        let _ = self.unregister();
    }
}

fn event_from_type<F>(event_type: u32, sample_time: F) -> io::Result<Option<PowerEvent>>
where
    F: FnOnce() -> io::Result<u64>,
{
    match event_type {
        PBT_APMSUSPEND => sample_time().map(|at_100ns| Some(PowerEvent::Suspending { at_100ns })),
        PBT_APMRESUMEAUTOMATIC
        | PBT_APMRESUMECRITICAL
        | PBT_APMRESUMESTANDBY
        | PBT_APMRESUMESUSPEND => Ok(Some(PowerEvent::Resumed)),
        _ => Ok(None),
    }
}

fn win32_result(status: u32) -> io::Result<()> {
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status as i32))
    }
}

unsafe extern "system" fn power_callback(
    context: *const c_void,
    event_type: u32,
    _setting: *const c_void,
) -> u32 {
    let Some(context) = (context as *const CallbackContext).as_ref() else {
        return ERROR_INVALID_PARAMETER;
    };

    let notification = match event_from_type(event_type, unbiased_time_100ns) {
        Ok(Some(event)) => Some(PowerNotification::Event(event)),
        Ok(None) => None,
        Err(error) => Some(PowerNotification::ClockError(error)),
    };

    if let Some(notification) = notification {
        let _ = context.sender.send(notification);
    }

    ERROR_SUCCESS
}

#[cfg(test)]
mod tests {
    use super::{event_from_type, win32_result, PowerEvent, PowerSubscription};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND,
    };

    #[test]
    fn suspend_event_captures_its_time_boundary() {
        let event = event_from_type(PBT_APMSUSPEND, || Ok(42)).unwrap();

        assert_eq!(event, Some(PowerEvent::Suspending { at_100ns: 42 }));
    }

    #[test]
    fn resume_events_do_not_sample_elapsed_time() {
        let automatic = event_from_type(PBT_APMRESUMEAUTOMATIC, || {
            panic!("resume must not sample the clock")
        })
        .unwrap();
        let user = event_from_type(PBT_APMRESUMESUSPEND, || {
            panic!("resume must not sample the clock")
        })
        .unwrap();

        assert_eq!(automatic, Some(PowerEvent::Resumed));
        assert_eq!(user, Some(PowerEvent::Resumed));
    }

    #[test]
    fn unrelated_power_events_are_ignored() {
        let event = event_from_type(u32::MAX, || panic!("unknown event must not sample")).unwrap();

        assert_eq!(event, None);
    }

    #[test]
    fn win32_errors_keep_their_status_code() {
        let error = win32_result(5).unwrap_err();

        assert_eq!(error.raw_os_error(), Some(5));
    }

    #[test]
    fn registration_can_be_cleanly_released() {
        let (subscription, _events) =
            PowerSubscription::register().expect("power callback registration should work");

        drop(subscription);
    }
}
