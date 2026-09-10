//! One explicit attachment selection or upload; paths never enter UI state or diagnostics.
use client_core::Command;
use discord_api::upload::{Source, Status};
use eframe::egui;
use model::Id;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use tokio::sync::watch;

pub struct UploadRequest {
    pub command: Command,
    pub source: Source,
    pub progress: watch::Sender<Status>,
    pub cancel: watch::Sender<bool>,
}

struct Choosing {
    result: mpsc::Receiver<Result<Option<Source>, &'static str>>,
    cancelled: Arc<AtomicBool>,
}
struct Uploading {
    progress: watch::Receiver<Status>,
    cancel: watch::Sender<bool>,
    cancelling: bool,
}
#[derive(Default)]
pub struct Uploads {
    scope: Option<(u64, Id)>,
    selected: Option<Source>,
    choosing: Option<Choosing>,
    uploading: Option<Uploading>,
    last: Option<Status>,
}
impl Uploads {
    pub fn start_choose(
        &mut self,
        generation: u64,
        channel: Id,
        runtime: &tokio::runtime::Handle,
        context: &egui::Context,
        parent: Arc<winit::window::Window>,
    ) -> Result<(), &'static str> {
        if self.busy() || self.selected.is_some() {
            return Err("Remove the current attachment or wait for its operation to finish");
        }
        // Construct on the native UI thread; await and inspect outside rendering.
        let dialog = platform::save::attachment_source(parent);
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let (send, result) = mpsc::sync_channel(1);
        let context = context.clone();
        runtime.spawn(async move {
            let result = async {
                let path = dialog.await;
                if flag.load(Ordering::Acquire) {
                    return Ok(None);
                }
                match path {
                    Some(path) => Source::inspect(path).await.map(Some),
                    None => Ok(None),
                }
            }
            .await;
            let _ = send.send(result);
            context.request_repaint();
        });
        self.scope = Some((generation, channel));
        self.last = None;
        self.choosing = Some(Choosing { result, cancelled });
        Ok(())
    }
    pub fn poll(
        &mut self,
        generation: u64,
        channel: Option<Id>,
        allowed: bool,
        context: &egui::Context,
    ) {
        if self
            .scope
            .is_some_and(|scope| !allowed || Some(scope) != channel.map(|id| (generation, id)))
        {
            self.remove();
        }
        if let Some(choosing) = &self.choosing {
            let result = match choosing.result.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Attachment chooser interrupted"))
                }
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                let cancelled = choosing.cancelled.load(Ordering::Acquire);
                self.choosing = None;
                if cancelled {
                    self.last = Some(Status::Cancelled);
                } else {
                    match result {
                        Ok(Some(source)) => {
                            self.selected = Some(source);
                            self.last = None;
                        }
                        Ok(None) => self.last = Some(Status::Cancelled),
                        Err(error) => self.last = Some(Status::Failed(error)),
                    }
                }
            }
        }
        if let Some(uploading) = &mut self.uploading {
            let status = uploading.progress.borrow_and_update().clone();
            let closed = uploading.progress.has_changed().is_err();
            self.last = Some(if closed {
                match status {
                    Status::Sending => Status::Failed(
                        "Message outcome unknown; check the conversation before retrying",
                    ),
                    Status::Preparing | Status::Uploading { .. } if uploading.cancelling => {
                        Status::Cancelled
                    }
                    Status::Preparing | Status::Uploading { .. } => {
                        Status::Failed("Attachment upload interrupted")
                    }
                    terminal => terminal,
                }
            } else {
                status
            });
            // Cancellation retains this slot until the actual network worker releases its sender.
            if closed {
                self.uploading = None;
            }
        }
        if self.busy() {
            context.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    pub fn selection(&self) -> Option<(&str, u64)> {
        self.selected
            .as_ref()
            .map(|source| (source.filename(), source.size()))
    }
    pub fn busy(&self) -> bool {
        self.choosing.is_some() || self.uploading.is_some()
    }
    pub fn has_unsent(&self) -> bool {
        self.selected.is_some() || self.busy()
    }
    pub fn status(&self) -> Option<String> {
        if let Some(choosing) = &self.choosing {
            return Some(
                if choosing.cancelled.load(Ordering::Acquire) {
                    "Attachment selection cancelled; close the file chooser"
                } else {
                    "Choosing attachment..."
                }
                .into(),
            );
        }
        if self.uploading.as_ref().is_some_and(|job| job.cancelling) {
            return Some("Cancelling upload; a message already sending may still arrive".into());
        }
        self.last.as_ref().map(|status| match status {
            Status::Preparing => "Preparing attachment...".into(),
            Status::Uploading { sent, total } => {
                format!("Uploading attachment: {sent} / {total} bytes")
            }
            Status::Sending => "Sending attachment message...".into(),
            Status::Finished => "Attachment message sent".into(),
            Status::Cancelled => "Attachment upload cancelled".into(),
            Status::Failed(error) => (*error).into(),
        })
    }
    pub fn remove(&mut self) {
        self.selected = None;
        self.cancel();
    }
    pub fn cancel(&mut self) {
        if let Some(choosing) = &self.choosing {
            choosing.cancelled.store(true, Ordering::Release);
        }
        if let Some(uploading) = &mut self.uploading {
            uploading.cancelling = true;
            uploading.cancel.send_replace(true);
        }
    }
    pub fn take_source(&mut self, generation: u64, channel: Id) -> Option<Source> {
        if self.scope != Some((generation, channel)) || self.busy() {
            return None;
        }
        self.selected.take()
    }
    pub fn begin_upload(
        &mut self,
        progress: watch::Receiver<Status>,
        cancel: watch::Sender<bool>,
    ) -> Result<(), &'static str> {
        if self.busy() || self.selected.is_some() || self.scope.is_none() {
            cancel.send_replace(true);
            return Err("Attachment operation already active or no selection scope");
        }
        self.last = Some(progress.borrow().clone());
        self.uploading = Some(Uploading {
            progress,
            cancel,
            cancelling: false,
        });
        Ok(())
    }
}
impl Drop for Uploads {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_chooser_and_upload_keep_the_single_slot_until_the_worker_finishes() {
        let context = egui::Context::default();
        let (send, result) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut uploads = Uploads {
            scope: Some((1, Id(2))),
            choosing: Some(Choosing {
                result,
                cancelled: cancelled.clone(),
            }),
            selected: None,
            uploading: None,
            last: None,
        };
        uploads.poll(2, Some(Id(2)), true, &context);
        assert!(cancelled.load(Ordering::Acquire));
        assert!(uploads.busy());
        assert!(send.send(Ok(None)).is_ok());
        uploads.poll(2, Some(Id(2)), true, &context);
        assert!(!uploads.busy());
        assert!(uploads.selection().is_none());

        uploads.scope = Some((2, Id(2)));
        let (progress, receive) = watch::channel(Status::Sending);
        let (cancel, cancellation) = watch::channel(false);
        assert!(uploads.begin_upload(receive, cancel).is_ok());
        uploads.poll(2, Some(Id(3)), true, &context);
        assert!(*cancellation.borrow());
        assert!(uploads.busy());
        drop(progress);
        uploads.poll(2, Some(Id(3)), true, &context);
        assert!(!uploads.busy());
        assert!(uploads.status().unwrap().contains("outcome unknown"));

        let (progress, receive) = watch::channel(Status::Preparing);
        let (cancel, _) = watch::channel(false);
        assert!(uploads.begin_upload(receive, cancel).is_ok());
        progress.send_replace(Status::Finished);
        uploads.poll(2, Some(Id(2)), true, &context);
        assert!(uploads.busy());
        drop(progress);
        uploads.poll(2, Some(Id(2)), true, &context);
        assert!(!uploads.busy());
        assert_eq!(uploads.status().as_deref(), Some("Attachment message sent"));
    }
}
