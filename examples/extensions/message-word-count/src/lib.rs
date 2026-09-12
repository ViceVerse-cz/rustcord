use serein_extension_sdk::{Element, Invocation, Output};

fn count(input: Invocation) -> Output {
	let words = input
		.selected_message
		.as_deref()
		.unwrap_or_default()
		.split_whitespace()
		.count();
	Output {
		panel: vec![Element::Text {
			text: format!("Words: {words}"),
		}],
		..Default::default()
	}
}

serein_extension_sdk::export!(count);

#[test]
fn counts_unicode_whitespace() {
	let output = count(Invocation {
		action: "count".into(),
		selected_message: Some("Hello\nworld\tčau".into()),
		..Default::default()
	});
	assert!(matches!(&output.panel[0],Element::Text{text} if text == "Words: 3"));
}
