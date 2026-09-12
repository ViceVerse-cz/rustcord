//! Minimal author-side ABI. This crate belongs inside the sandbox, not in the host.
use serde::{Deserialize, Serialize};
pub use serde_json;
use std::collections::BTreeMap;

#[derive(Default, Deserialize)]
pub struct Invocation {
	pub action: String,
	#[serde(default)]
	pub selected_message: Option<String>,
	#[serde(default)]
	pub composer: Option<String>,
	#[serde(default)]
	pub storage: Option<String>,
	#[serde(default)]
	pub values: BTreeMap<String, String>,
}

#[derive(Default, Serialize)]
pub struct Output {
	pub preserve_deleted_messages: bool,
	pub replacement: Option<String>,
	pub panel: Vec<Element>,
	pub storage: Option<String>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Element {
	Text {
		text: String,
	},
	Row {
		children: Vec<Element>,
	},
	Button {
		id: String,
		label: String,
	},
	TextInput {
		id: String,
		label: String,
		value: String,
	},
	Checkbox {
		id: String,
		label: String,
		checked: bool,
	},
}

/// Export a plain `fn(Invocation) -> Output` as Serein ABI version 1.
/// Each invocation gets a fresh instance, so buffers are reclaimed when it finishes.
#[macro_export]
macro_rules! export {
	($handler:path) => {
		#[unsafe(no_mangle)]
		pub extern "C" fn serein_alloc(length: u32) -> u32 {
			if length > 256 * 1024 {
				return 0;
			}
			let buffer = vec![0_u8; length as usize].into_boxed_slice();
			Box::into_raw(buffer) as *mut u8 as u32
		}

		/// # Safety
		/// The Serein host supplies the pointer returned by `serein_alloc` and its allocated length.
		#[unsafe(no_mangle)]
		pub unsafe extern "C" fn serein_invoke(pointer: u32, length: u32) -> u64 {
			if length > 256 * 1024 {
				return 0;
			}
			// SAFETY: the host ABI owns this allocation and writes exactly `length` bytes.
			let input =
				unsafe { std::slice::from_raw_parts(pointer as *const u8, length as usize) };
			let Ok(input) = $crate::serde_json::from_slice::<$crate::Invocation>(input) else {
				return 0;
			};
			let Ok(output) = $crate::serde_json::to_vec(&$handler(input)) else {
				return 0;
			};
			if output.len() > 256 * 1024 {
				return 0;
			}
			let length = output.len() as u64;
			let pointer = Box::into_raw(output.into_boxed_slice()) as *mut u8 as u32;
			((pointer as u64) << 32) | length
		}
	};
}
