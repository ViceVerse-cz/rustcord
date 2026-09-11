//! Offline native framebuffer capture; no account, filesystem cache, or network adapters.
use eframe::egui;
use std::{
	path::PathBuf,
	sync::{
		Arc,
		atomic::{AtomicBool, Ordering},
	},
	thread::JoinHandle,
	time::{Duration, Instant},
};

struct Preview {
	messaging: ui::MessagingUi,
	state: client_core::State,
	output: PathBuf,
	frames: u8,
	requested: bool,
	screenshot: Option<std::sync::mpsc::Receiver<Arc<egui::ColorImage>>>,
	writer: Option<JoinHandle<Result<(), String>>>,
	saved: Arc<AtomicBool>,
	started: Instant,
}

impl eframe::App for Preview {
	fn persist_egui_memory(&self) -> bool {
		false
	}

	fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
		let ctx = ui.ctx().clone();
		ui::design::paint_backdrop(&ctx);
		// The fixture is already loaded. Commands are deliberately never sent to adapters.
		let _ = self.messaging.show(ui, &mut self.state);
		if self.requested && self.writer.is_none() {
			let screenshot = self
				.screenshot
				.as_ref()
				.and_then(|receiver| receiver.try_recv().ok());
			if let Some(image) = screenshot {
				let output = self.output.clone();
				self.writer = Some(std::thread::spawn(move || {
					if image.size[0] > 4096 || image.size[1] > 4096 {
						return Err("Screenshot exceeds the 4096-pixel dimension limit".into());
					}
					let pixels: Vec<u8> = image
						.pixels
						.iter()
						.flat_map(|pixel| pixel.to_srgba_unmultiplied())
						.collect();
					image::save_buffer_with_format(
						&output,
						&pixels,
						image.size[0] as u32,
						image.size[1] as u32,
						image::ColorType::Rgba8,
						image::ImageFormat::Png,
					)
					.map_err(|error| error.to_string())
				}));
			}
		}
		if self.writer.as_ref().is_some_and(JoinHandle::is_finished) {
			match self.writer.take().unwrap().join() {
				Ok(Ok(())) => {
					self.saved.store(true, Ordering::Release);
					println!(
						"Saved offline native framebuffer: {}",
						self.output.display()
					);
				}
				Ok(Err(error)) => eprintln!("Screenshot save failed: {error}"),
				Err(_) => eprintln!("Screenshot worker failed"),
			}
			ctx.send_viewport_cmd(egui::ViewportCommand::Close);
			return;
		}
		if self.started.elapsed() > Duration::from_secs(20) {
			eprintln!("Native screenshot callback did not complete within 20 seconds");
			ctx.send_viewport_cmd(egui::ViewportCommand::Close);
			return;
		}
		self.frames = self.frames.saturating_add(1);
		if self.frames >= 5 && !self.requested {
			self.requested = true;
			let (send, receive) = std::sync::mpsc::sync_channel(1);
			self.screenshot = Some(receive);
			let wake = ctx.clone();
			ctx.request_screenshot(move |image| {
				let _ = send.try_send(image);
				wake.request_repaint();
			});
		}
		ctx.request_repaint_after(Duration::from_millis(100));
	}
}

fn prime_profile(state: &mut client_core::State) {
	if let Some(client_core::Command::EditProfile { user, request, .. }) = state.load_own_profile()
	{
		let profile = ui::synthetic_own_profile(state.user.as_ref().unwrap());
		state.apply(client_core::Envelope {
			generation: state.generation,
			event: client_core::Event::ProfileEdited {
				user,
				request,
				result: Ok(Box::new(profile)),
			},
		});
	}
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let args: Vec<_> = std::env::args().skip(1).collect();
	let value = |prefix: &str| args.iter().find_map(|arg| arg.strip_prefix(prefix));
	if !args.iter().any(|arg| arg == "--demo") {
		return Err("Usage: profile_preview --demo --output=PATH.png [--page=profile|account] [--width=1120] [--height=760] [--light]".into());
	}
	let output = PathBuf::from(value("--output=").ok_or("Missing --output=PATH.png")?);
	let page = value("--page=").unwrap_or("profile").to_owned();
	if !matches!(page.as_str(), "profile" | "account") {
		return Err("Page must be profile or account".into());
	}
	let width: f32 = value("--width=").unwrap_or("1120").parse()?;
	let height: f32 = value("--height=").unwrap_or("760").parse()?;
	if !(500.0..=1920.0).contains(&width) || !(520.0..=1200.0).contains(&height) {
		return Err("Viewport must be 500-1920 by 520-1200".into());
	}
	let light = args.iter().any(|arg| arg == "--light");
	let saved = Arc::new(AtomicBool::new(false));
	let completed = saved.clone();
	eframe::run_native(
		"Serein · offline profile preview",
		eframe::NativeOptions {
			viewport: egui::ViewportBuilder::default()
				.with_inner_size([width, height])
				.with_decorations(false),
			renderer: eframe::Renderer::Wgpu,
			persist_window: false,
			..Default::default()
		},
		Box::new(move |cc| {
			ui::fonts::install(&cc.egui_ctx);
			ui::design::apply(&cc.egui_ctx);
			cc.egui_ctx.set_theme(if light {
				egui::ThemePreference::Light
			} else {
				egui::ThemePreference::Dark
			});
			let mut state = test_support::demo_state();
			if page == "profile" {
				prime_profile(&mut state);
			}
			let mut messaging = ui::MessagingUi::default();
			messaging.preview_settings(&page);
			Ok(Box::new(Preview {
				messaging,
				state,
				output,
				frames: 0,
				requested: false,
				screenshot: None,
				writer: None,
				saved: completed,
				started: Instant::now(),
			}))
		}),
	)?;
	if !saved.load(Ordering::Acquire) {
		return Err("No screenshot saved".into());
	}
	Ok(())
}
