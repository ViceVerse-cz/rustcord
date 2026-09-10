fn main() {
	println!("cargo:rerun-if-changed=../../packaging/macos/Info.plist");
	if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos")
		&& std::env::var_os("CARGO_FEATURE_VOICE").is_some()
	{
		// ScreenCaptureKit's Swift bridge uses the system Swift runtime.
		println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
		// cargo run has no app bundle: embed the same microphone usage description.
		let plist = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
			.join("../../packaging/macos/Info.plist");
		println!(
			"cargo:rustc-link-arg-bin=serein=-Wl,-sectcreate,__TEXT,__info_plist,{}",
			plist.display()
		);
	}
}
