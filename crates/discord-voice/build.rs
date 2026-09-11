fn main() {
	if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
		// Dependency link-args do not propagate to this crate's test executables.
		println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
	}
}
