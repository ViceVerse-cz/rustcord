//! The only credential-persistence boundary; the only webview is a temporary login surface.
pub mod save;
use client_core::auth::{Failure, SessionSecret};
use std::{
    sync::{
        Arc,
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};
use wry::{WebView, WebViewBuilder};

const SERVICE: &str = "org.serein.desktop";
const ACCOUNT: &str = "discord-session";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialError {
    Unavailable,
    Invalid,
    TimedOut,
}
pub fn load_session() -> Result<Option<SessionSecret>, CredentialError> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT).map_err(|_| CredentialError::Unavailable)?;
    match entry.get_password() {
        Ok(value) => SessionSecret::from_owner_input(value)
            .map(Some)
            .map_err(|_| CredentialError::Invalid),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(CredentialError::Unavailable),
    }
}
pub fn save_session(secret: &SessionSecret) -> Result<(), CredentialError> {
    keyring::Entry::new(SERVICE, ACCOUNT)
        .and_then(|entry| entry.set_password(secret.expose()))
        .map_err(|_| CredentialError::Unavailable)
}
pub fn forget_session() -> Result<(), CredentialError> {
    match keyring::Entry::new(SERVICE, ACCOUNT).and_then(|entry| entry.delete_credential()) {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(CredentialError::Unavailable),
    }
}
fn discord_origin(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str() == Some("discord.com")
            && url.port_or_known_default() == Some(443)
            && url.username().is_empty()
            && url.password().is_none()
    })
}
/// Receives only the account token used by THIS ephemeral, owner-operated login page.
/// No browser-profile reads, password interception, console instructions, or QR exchange implementation.
pub struct LoginView {
    view: WebView,
    tokens: Receiver<SessionSecret>,
    opened: Instant,
    #[cfg(target_os = "linux")]
    window: gtk::Window,
}
impl LoginView {
    pub fn open(
        parent: Arc<winit::window::Window>,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, Failure> {
        let (send, tokens) = mpsc::sync_channel(1);
        let mut random = [0_u8; 32];
        getrandom::fill(&mut random).map_err(|_| Failure::Protocol)?;
        let capability = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
            + ":";
        let script =
            include_str!("login-handoff.js").replace("__SEREIN_LOGIN_CAPABILITY__", &capability);
        let builder = WebViewBuilder::new()
            .with_url("https://discord.com/login")
            .with_incognito(true)
            .with_devtools(false)
            .with_initialization_script_for_main_only(script, true)
            .with_navigation_handler(|url| discord_origin(&url))
            .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
            .with_download_started_handler(|_, _| false)
            .with_ipc_handler(move |request| {
                if !discord_origin(&request.uri().to_string()) || request.body().len() > 2113 {
                    return;
                }
                let body = zeroize::Zeroizing::new(request.into_body());
                let Some(value) = body.strip_prefix(&capability) else {
                    return;
                };
                if let Ok(secret) = SessionSecret::from_owner_input(value.to_owned()) {
                    let _ = send.try_send(secret);
                    wake();
                }
            });
        #[cfg(not(target_os = "linux"))]
        let view = builder
            .with_bounds(bounds(&parent))
            .build_as_child(parent.as_ref())
            .map_err(|_| Failure::Protocol)?;
        #[cfg(target_os = "linux")]
        let (view, window) = {
            use gtk::prelude::*;
            use wry::WebViewBuilderExtUnix;
            gtk::init().map_err(|_| Failure::Protocol)?;
            let window = gtk::Window::new(gtk::WindowType::Toplevel);
            window.set_title("Discord sign-in · Serein");
            window.set_default_size(900, 700);
            let view = builder.build_gtk(&window).map_err(|_| Failure::Protocol)?;
            window.show_all();
            let _ = parent;
            (view, window)
        };
        Ok(Self {
            view,
            tokens,
            opened: Instant::now(),
            #[cfg(target_os = "linux")]
            window,
        })
    }
    pub fn token(&self) -> Option<SessionSecret> {
        self.tokens.try_recv().ok()
    }
    pub fn expired(&self) -> bool {
        self.opened.elapsed() > Duration::from_secs(600)
    }
    pub fn resize(&self, parent: &winit::window::Window) {
        #[cfg(not(target_os = "linux"))]
        let _ = self.view.set_bounds(bounds(parent));
        #[cfg(target_os = "linux")]
        {
            let _ = (parent, &self.view);
        }
    }
    pub fn pump(&self) {
        #[cfg(target_os = "linux")]
        for _ in 0..16 {
            if !gtk::events_pending() {
                break;
            }
            gtk::main_iteration_do(false);
        }
    }
}
#[cfg(target_os = "linux")]
impl Drop for LoginView {
    fn drop(&mut self) {
        use gtk::prelude::*;
        self.window.close();
    }
}
#[cfg(not(target_os = "linux"))]
fn bounds(parent: &winit::window::Window) -> wry::Rect {
    let size = parent.inner_size();
    let header = (90.0 * parent.scale_factor()) as u32;
    wry::Rect {
        position: wry::dpi::PhysicalPosition::new(0, header as i32).into(),
        size: wry::dpi::PhysicalSize::new(size.width, size.height.saturating_sub(header)).into(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handoff_accepts_only_our_discord_origin() {
        assert!(discord_origin("https://discord.com/login"));
        for value in [
            "http://discord.com",
            "https://discord.com.evil.test",
            "https://evil.test/discord.com",
            "https://user@discord.com",
            "https://discord.com:444",
        ] {
            assert!(!discord_origin(value));
        }
        let script = include_str!("login-handoff.js");
        assert!(!script.contains("localStorage"));
        assert!(!script.contains("password"));
    }
}
