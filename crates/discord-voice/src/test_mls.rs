//! Synthetic local MLS delivery service; never connects to Discord.
use crate::crypto::Dave;
use openmls::prelude::tls_codec::Serialize as _;
use openmls::prelude::*;
use openmls_basic_credential::SignatureKeyPair;
use openmls_rust_crypto::OpenMlsRustCrypto;

pub struct Delivery {
    signer: SignatureKeyPair,
    pub external: Vec<u8>,
}
impl Delivery {
    pub fn new() -> Self {
        let signer = SignatureKeyPair::new(SignatureScheme::ECDSA_SECP256R1_SHA256).unwrap();
        let external = ExternalSender::new(
            signer.public().to_vec().into(),
            BasicCredential::new(b"local test voice server".to_vec()).into(),
        )
        .tls_serialize_detached()
        .unwrap();
        Self { signer, external }
    }
    pub fn add(&self, creator: &mut Dave, key_package: &[u8]) -> (Vec<u8>, Vec<u8>) {
        assert_eq!(key_package[0], 26);
        let incoming = MlsMessageIn::tls_deserialize_exact_bytes(&key_package[1..]).unwrap();
        let MlsMessageBodyIn::KeyPackage(package) = incoming.extract() else {
            panic!("not a key package")
        };
        let provider = OpenMlsRustCrypto::default();
        let package = package
            .validate(provider.crypto(), ProtocolVersion::Mls10)
            .unwrap();
        let group = creator.session.group().unwrap();
        let proposal = ExternalProposal::new_add::<OpenMlsRustCrypto>(
            package,
            group.group_id().clone(),
            group.epoch(),
            &self.signer,
            SenderExtensionIndex::new(0),
        )
        .unwrap()
        .tls_serialize_detached()
        .unwrap();
        let mut payload = vec![0];
        payload.extend(VLBytes::new(proposal).tls_serialize_detached().unwrap());
        let result = creator.proposals(&payload).unwrap().unwrap();
        assert_eq!(result[0], 28);
        let (_, welcome) = MlsMessageIn::tls_deserialize_bytes(&result[1..]).unwrap();
        let commit_len = result.len() - 1 - welcome.len();
        (result[1..1 + commit_len].to_vec(), welcome.to_vec())
    }
}
#[test]
fn dave_two_parties_encrypt_decrypt_reject_tampering_and_transition_gate() {
    let server = Delivery::new();
    let mut alice = Dave::new(1, 2, 3).unwrap();
    let mut bob = Dave::new(2, 1, 3).unwrap();
    alice.session.set_external_sender(&server.external).unwrap();
    bob.session.set_external_sender(&server.external).unwrap();
    let package = bob.key_package().unwrap();
    let (commit, welcome) = server.add(&mut alice, &package);
    let mut committed = vec![0, 5];
    committed.extend(commit);
    let mut welcomed = vec![0, 5];
    welcomed.extend(welcome);
    assert_eq!(alice.group_changed(29, &committed).unwrap(), 5);
    assert_eq!(bob.group_changed(30, &welcomed).unwrap(), 5);
    assert!(!alice.ready);
    assert!(!bob.ready);
    alice.execute(5).unwrap();
    bob.execute(5).unwrap();
    let encrypted = alice
        .session
        .encrypt_opus(b"synthetic opus bytes")
        .unwrap()
        .into_owned();
    assert_ne!(encrypted, b"synthetic opus bytes");
    let mut corrupt = encrypted.clone();
    corrupt[0] ^= 1;
    assert!(
        bob.session
            .decrypt(1, davey::MediaType::AUDIO, &corrupt)
            .is_err()
    );
    assert_eq!(
        bob.session
            .decrypt(1, davey::MediaType::AUDIO, &encrypted)
            .unwrap(),
        b"synthetic opus bytes"
    );
    assert!(
        bob.session
            .decrypt(1, davey::MediaType::AUDIO, &encrypted)
            .is_err()
    );
    assert_eq!(
        alice.session.voice_privacy_code(),
        bob.session.voice_privacy_code()
    );
    bob.reset().unwrap();
    assert!(!bob.ready);
    assert!(bob.session.encrypt_opus(b"private voice").is_err());
}

#[test]
fn welcome_cannot_add_an_unexpected_dm_peer() {
    let server = Delivery::new();
    let mut alice = Dave::new(1, 2, 3).unwrap();
    let mut bob = Dave::new(2, 99, 3).unwrap();
    alice.session.set_external_sender(&server.external).unwrap();
    bob.session.set_external_sender(&server.external).unwrap();
    let package = bob.key_package().unwrap();
    let (_, welcome) = server.add(&mut alice, &package);
    let mut welcomed = vec![0, 0];
    welcomed.extend(welcome);
    assert!(bob.group_changed(30, &welcomed).is_err());
    assert!(!bob.ready);
    assert!(bob.execute(0).is_err());
}
