//! Offline sandbox timing: `cargo run --locked --release -p extensions --example benchmark`.
//! Measures wall time, not process memory or native UI frame time.
use extensions::{Invocation, invoke, parse_package};
use std::time::Instant;

fn median_us(mut measurements: Vec<u128>) -> u128 {
	measurements.sort_unstable();
	measurements[measurements.len() / 2]
}

fn measure(name: &str, bytes: &[u8], invocation: Invocation) {
	let package = parse_package(bytes).expect("synthetic package is valid");
	std::hint::black_box(invoke(&package, &invocation).expect("warmup succeeds"));
	let mut cold = Vec::new();
	for _ in 0..5 {
		let start = Instant::now();
		let package = parse_package(bytes).expect("synthetic package is valid");
		std::hint::black_box(invoke(&package, &invocation).expect("invocation succeeds"));
		cold.push(start.elapsed().as_micros());
	}
	let mut repeated = Vec::new();
	for _ in 0..100 {
		let start = Instant::now();
		std::hint::black_box(invoke(&package, &invocation).expect("invocation succeeds"));
		repeated.push(start.elapsed().as_micros());
	}

	println!(
		"{name}: package_bytes={}, wasm_bytes={}, parse_plus_invoke_median_us={} (n=5), invoke_median_us={} (n=100; new runtime each call)",
		bytes.len(),
		package.wasm.len(),
		median_us(cold),
		median_us(repeated)
	);
}

fn main() {
	measure(
		"message-delete-protector",
		include_bytes!(
			"../../../examples/extensions/packages/message-delete-protector.serein-extension"
		),
		Invocation {
			action: "activate".into(),
			..Default::default()
		},
	);
}
