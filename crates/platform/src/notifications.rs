//! Session-opt-in OS alerts. No account identifiers or message content cross this boundary.
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc::{self, Receiver, SyncSender},
};

// Eight fixed-size commands (128 bytes at most); overflow drops an alert, never message state.
const QUEUE_ITEMS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Status {
    Disabled,
    Enabling,
    Ready,
    Denied,
    Unavailable,
    QueueFull,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Disabled => "System notifications are off for this session.",
            Self::Enabling => "Checking system notification permission…",
            Self::Ready => "System notifications enabled; message content stays hidden.",
            Self::Denied => "System notifications are disabled in your OS settings.",
            Self::QueueFull => {
                "Notification queue full; an alert was skipped. Unread indicators are retained."
            }
            Self::Unavailable => {
                #[cfg(target_os = "macos")]
                {
                    "System notifications unavailable. Run the packaged Serein.app and check System Settings > Notifications."
                }
                #[cfg(target_os = "windows")]
                {
                    "System notifications unavailable. Register the packaged Start Menu shortcut with install-notifications.ps1 and check Windows notification settings."
                }
                #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                {
                    "System notifications unavailable. Check your desktop notification service and settings."
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Command {
    generation: u64,
    alert: bool,
}

/// Created without OS calls or a thread. The worker starts only after explicit opt-in.
pub struct Notifications {
    send: Option<SyncSender<Command>>,
    generation: Arc<AtomicU64>,
    status: Arc<AtomicU64>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Notifications {
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            send: None,
            generation: Arc::new(AtomicU64::new(0)),
            status: Arc::new(AtomicU64::new(0)),
            wake: Arc::new(wake),
        }
    }

    /// Enable for this session only. macOS may ask for OS permission; never blocks rendering.
    pub fn set_enabled(&mut self, enabled: bool) {
        if (self.generation.load(Ordering::Acquire) & 1 != 0) == enabled {
            return;
        }
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.status.store(
            encoded(
                generation,
                if enabled {
                    Status::Enabling
                } else {
                    Status::Disabled
                },
            ),
            Ordering::Release,
        );
        if enabled && self.send.is_none() {
            let (send, receive) = mpsc::sync_channel(QUEUE_ITEMS);
            let current = Arc::clone(&self.generation);
            let status = Arc::clone(&self.status);
            let wake = Arc::clone(&self.wake);
            if std::thread::Builder::new()
                .name("serein-notifications".into())
                .spawn(move || worker(receive, current, status, wake))
                .is_err()
            {
                self.status
                    .store(encoded(generation, Status::Unavailable), Ordering::Release);
                return;
            }
            self.send = Some(send);
        }
        // If full, the next queued command also observes the changed generation and clears.
        if let Some(send) = &self.send {
            let _ = send.try_send(Command {
                generation,
                alert: false,
            });
        }
    }

    /// Cancel queued alerts and close the outstanding alert where supported. OS history may remain.
    pub fn clear(&mut self) {
        self.set_enabled(false);
    }

    /// Invalidate pending/outstanding alerts after read, mute, DND or connection changes.
    /// Retains the session opt-in and does not request OS authorization again.
    pub fn dismiss(&mut self) {
        let result = match self.status() {
            Status::QueueFull => Status::Ready,
            result => result,
        };
        let generation = self.generation.fetch_add(2, Ordering::AcqRel) + 2;
        self.status
            .store(encoded(generation, result), Ordering::Release);
        if let Some(send) = &self.send {
            let _ = send.try_send(Command {
                generation,
                alert: false,
            });
        }
    }

    /// Queue a privacy-preserving generic alert. False means disabled, unavailable or overloaded.
    pub fn notify(&self) -> bool {
        if !matches!(self.status(), Status::Ready | Status::QueueFull) {
            return false;
        }
        let generation = self.generation.load(Ordering::Acquire);
        let Some(send) = &self.send else { return false };
        match send.try_send(Command {
            generation,
            alert: true,
        }) {
            Ok(()) => true,
            Err(error) => {
                let status = match error {
                    mpsc::TrySendError::Full(_) => Status::QueueFull,
                    mpsc::TrySendError::Disconnected(_) => Status::Unavailable,
                };
                self.status
                    .store(encoded(generation, status), Ordering::Release);
                false
            }
        }
    }

    pub fn status(&self) -> Status {
        let generation = self.generation.load(Ordering::Acquire);
        let value = self.status.load(Ordering::Acquire);
        if value >> 8 != generation {
            return if generation & 1 == 0 {
                Status::Disabled
            } else {
                Status::Enabling
            };
        }
        match value & 255 {
            0 => Status::Disabled,
            1 => Status::Enabling,
            2 => Status::Ready,
            3 => Status::Denied,
            5 => Status::QueueFull,
            _ => Status::Unavailable,
        }
    }
}

impl Drop for Notifications {
    fn drop(&mut self) {
        self.clear();
        // Dropping the sole sender wakes the worker; never join an OS call on the UI thread.
    }
}

fn encoded(generation: u64, status: Status) -> u64 {
    (generation << 8) | status as u64
}

fn worker(
    receive: Receiver<Command>,
    current: Arc<AtomicU64>,
    status: Arc<AtomicU64>,
    wake: Arc<dyn Fn() + Send + Sync>,
) {
    let mut generation = 0;
    let mut outcome = Status::Disabled;
    let mut outstanding = None;
    while let Ok(command) = receive.recv() {
        let active = current.load(Ordering::Acquire);
        if active != generation {
            close(&mut outstanding);
            let was_enabled = generation & 1 != 0;
            generation = active;
            outcome = if generation & 1 == 0 {
                Status::Disabled
            } else if !was_enabled
                || status.load(Ordering::Acquire) == encoded(generation, Status::Enabling)
            {
                authorize()
            } else {
                outcome
            };
            publish(&status, generation, outcome);
            wake();
        }
        if outcome != Status::Ready
            || !command.alert
            || command.generation != generation
            || current.load(Ordering::Acquire) != generation
        {
            continue;
        }
        // Keep at most one generic notification/response handle, including in OS history where supported.
        close(&mut outstanding);
        outcome = match show() {
            Ok(handle) => {
                outstanding = Some(handle);
                Status::Ready
            }
            Err(()) => Status::Unavailable,
        };
        if current.load(Ordering::Acquire) != generation {
            close(&mut outstanding);
        }
        publish(&status, generation, outcome);
        wake();
    }
    close(&mut outstanding);
}

fn publish(status: &AtomicU64, generation: u64, outcome: Status) {
    // An OS result must not overwrite the next session/dismissal's state.
    let _ = status.fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
        (current >> 8 == generation).then_some(encoded(generation, outcome))
    });
}

fn authorize() -> Status {
    #[cfg(target_os = "macos")]
    {
        match notify_rust::request_auth_blocking() {
            Ok(true) => Status::Ready,
            Ok(false) => Status::Denied,
            Err(_) => Status::Unavailable,
        }
    }
    #[cfg(target_os = "windows")]
    {
        use windows::{
            UI::Notifications::{NotificationSetting, ToastNotificationManager},
            core::HSTRING,
        };
        match ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(
            "org.serein.desktop",
        ))
        .and_then(|notifier| notifier.Setting())
        {
            Ok(NotificationSetting::Enabled) => Status::Ready,
            Ok(_) => Status::Denied,
            Err(_) => Status::Unavailable,
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Status::Ready
    }
}

fn show() -> Result<notify_rust::NotificationHandle, ()> {
    #[cfg(target_os = "macos")]
    {
        // The blocking wrapper mistakes a busy AppKit run loop for a stopped one.
        // Await the OS completion on this worker; never block the native UI thread.
        futures_lite::future::block_on(notification().show_async()).map_err(|_| ())
    }
    #[cfg(not(target_os = "macos"))]
    notification().show().map_err(|_| ())
}

fn notification() -> notify_rust::Notification {
    let mut notification = notify_rust::Notification::new();
    notification
        .appname("Serein")
        .summary("Serein")
        .body("You have a new message.")
        .timeout(5_000);
    #[cfg(target_os = "windows")]
    notification.app_id("org.serein.desktop");
    notification
}

fn close(outstanding: &mut Option<notify_rust::NotificationHandle>) {
    if let Some(handle) = outstanding.take() {
        #[cfg(not(target_os = "windows"))]
        handle.close();
        #[cfg(target_os = "windows")]
        {
            use windows::{UI::Notifications::ToastNotificationManager, core::HSTRING};
            drop(handle);
            let _ = ToastNotificationManager::History()
                .and_then(|history| history.ClearWithId(&HSTRING::from("org.serein.desktop")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_is_lazy_and_fixed_queue_is_bounded_and_invalidated() {
        let alert = notification();
        assert_eq!(alert.summary, "Serein");
        assert_eq!(alert.body, "You have a new message.");
        let mut notifications = Notifications::new(|| {});
        assert_eq!(notifications.status(), Status::Disabled);
        assert!(notifications.send.is_none());
        assert!(!notifications.notify());
        // Substitute only the queue; tests never authorize or contact an OS notification service.
        let (send, receive) = mpsc::sync_channel(QUEUE_ITEMS);
        notifications.send = Some(send);
        notifications.generation.store(1, Ordering::Release);
        notifications
            .status
            .store(encoded(1, Status::Ready), Ordering::Release);
        for _ in 0..QUEUE_ITEMS {
            assert!(notifications.notify());
        }
        assert!(!notifications.notify());
        assert_eq!(notifications.status(), Status::QueueFull);
        notifications.dismiss();
        assert_eq!(notifications.status(), Status::Ready);
        for command in receive.try_iter().filter(|command| command.alert) {
            assert_ne!(
                command.generation,
                notifications.generation.load(Ordering::Acquire)
            );
        }
        assert!(notifications.notify());
        notifications.clear();
        assert_eq!(notifications.status(), Status::Disabled);
        assert!(!notifications.notify());
        for command in receive.try_iter().filter(|command| command.alert) {
            assert_ne!(
                command.generation,
                notifications.generation.load(Ordering::Acquire)
            );
        }
        assert!(std::mem::size_of::<Command>() * QUEUE_ITEMS <= 128);
        let active_status = notifications.status.load(Ordering::Acquire);
        publish(&notifications.status, 1, Status::Unavailable);
        assert_eq!(notifications.status.load(Ordering::Acquire), active_status);
        assert_eq!(notifications.status(), Status::Disabled);
    }
}
