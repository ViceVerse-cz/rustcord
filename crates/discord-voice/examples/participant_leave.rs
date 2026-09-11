//! Offline debug check of DM departure and encrypted rejoin; no network or audio devices.
#[allow(dead_code)]
#[path = "../src/crypto.rs"]
mod crypto;
#[allow(dead_code)]
#[path = "../src/test_mls.rs"]
mod test_mls;

use crypto::{Dave, Identity};

fn main() {
	let server = test_mls::Delivery::new();
	let mut alice = Dave::with_identity(1, Some(2), 3, Identity::generate()).unwrap();
	let mut bob = Dave::with_identity(2, Some(1), 3, Identity::generate()).unwrap();
	alice.session.set_external_sender(&server.external).unwrap();
	bob.session.set_external_sender(&server.external).unwrap();
	for _ in 0..2 {
		let (commit, welcome) = server.add(&mut alice, &bob.key_package().unwrap());
		alice
			.group_changed(29, &[&[0, 0], commit.as_slice()].concat())
			.unwrap();
		bob.group_changed(30, &[&[0, 0], welcome.as_slice()].concat())
			.unwrap();
		assert!(alice.ready && bob.ready);
		let encrypted = alice
			.session
			.encrypt_opus(b"synthetic voice")
			.unwrap()
			.into_owned();
		assert_eq!(
			bob.session
				.decrypt(1, davey::MediaType::AUDIO, &encrypted)
				.unwrap(),
			b"synthetic voice"
		);
		assert!(alice.disconnect(2).unwrap());
		assert!(!alice.ready && !alice.contains(2));
		assert!(!alice.disconnect(2).unwrap());
		assert!(alice.wait_for_peer().is_err()); // Old group still contains the departed peer.
		alice.reinitialize().unwrap();
		alice.wait_for_peer().unwrap(); // Discord's transition 0 while alone.
		assert!(alice.waiting && !alice.ready);
		assert!(alice.session.encrypt_opus(b"never plaintext").is_err());
		assert!(alice.connect(&[4]).is_err()); // Departure never widens the DM allowlist.
		assert!(alice.disconnect(1).is_err());
		assert!(alice.connect(&[2]).unwrap());
		assert!(!alice.waiting && !alice.ready);
		assert!(alice.wait_for_peer().is_err());
		bob.reinitialize().unwrap();
	}
	println!("DM departure stays joined with media paused; encrypted rejoin passed twice.");
}
