use serein_extension_sdk::{Invocation, Output};

fn transform(input: Invocation) -> Output {
	Output {
		replacement: input.composer.map(|text| text.to_uppercase()),
		..Default::default()
	}
}

serein_extension_sdk::export!(transform);

#[test]
fn transforms_unicode_without_sending() {
	let output = transform(Invocation {
		action: "uppercase".into(),
		composer: Some("Hello, čau!".into()),
		..Default::default()
	});
	assert_eq!(output.replacement.as_deref(), Some("HELLO, ČAU!"));
}
