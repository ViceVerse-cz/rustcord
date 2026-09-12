//! Offline debug check: cargo run --locked -p serein --example extension_settings
use eframe::egui;

fn texts(shape: &egui::Shape, labels: &mut Vec<String>) {
	match shape {
		egui::Shape::Text(text) => labels.push(text.galley.job.text.clone()),
		egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| texts(shape, labels)),
		_ => {}
	}
}
fn main() {
	let ctx = egui::Context::default();
	ui::fonts::install(&ctx);
	ui::design::apply(&ctx);
	let mut state = test_support::demo_state();
	let mut messaging = ui::MessagingUi::default();
	let packages: [&[u8]; 2] = [
		include_bytes!(
			"../../../examples/extensions/packages/message-delete-protector.serein-extension"
		),
		include_bytes!("../../../extensions/ocean.serein-extension"),
	];
	messaging.extensions.set_entries(
		packages
			.into_iter()
			.map(|bytes| {
				let package = extensions::parse_package(bytes).unwrap();
				ui::ExtensionEntry {
					manifest: package.manifest,
					theme_preview: package.theme,
					description: String::new(),
					preview: None,
					reviewed: true,
					sha256: "a".repeat(64),
					download_bytes: bytes.len() as u64,
					enabled: false,
					cleanup_pending: false,
					update_available: false,
					update_manifest: None,
				}
			})
			.collect(),
	);
	for (page, visible, hidden) in [
		("extensions", "Message delete protector", "Ocean"),
		("themes", "Ocean", "Message delete protector"),
	] {
		messaging.preview_settings(page);
		let mut labels = Vec::new();
		for _ in 0..3 {
			labels.clear();
			let output = ctx.run_ui(
				egui::RawInput {
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(1120.0, 760.0),
					)),
					..Default::default()
				},
				|ui| {
					let _ = messaging.show(ui, &mut state);
				},
			);
			for shape in &output.shapes {
				texts(&shape.shape, &mut labels);
			}
			output.drop_without_applying_deltas();
		}
		assert!(labels.iter().any(|text| text == visible));
		assert!(!labels.iter().any(|text| text == hidden));
		assert!(
			labels.iter().any(|text| text == "Extensions")
				&& labels.iter().any(|text| text == "Themes")
		);
		assert!(labels.iter().any(|text| text == &format!("Search {page}")));
		assert!(
			!labels
				.iter()
				.any(|text| text.contains("Make Serein yours") || text.contains("A new look."))
		);
	}
	println!(
		"Extension settings debug check passed: separate pages, filtered cards, search fields, no intro banner."
	);
}
