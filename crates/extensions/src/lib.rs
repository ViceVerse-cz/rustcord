//! Versioned, bounded local extensions. No Discord or native platform access is exposed.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

mod runtime;
pub use runtime::invoke;

pub const API_VERSION: u32 = 1;
pub const MAX_PACKAGE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_MODULE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_CATALOG_BYTES: usize = 1024 * 1024;
pub const MAX_IO_BYTES: usize = 256 * 1024;
pub const MAX_STORAGE_BYTES: usize = 1024 * 1024;
pub const MAX_PLUGINS: usize = 8;
pub const MAX_PANEL_ELEMENTS: usize = 64;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("Extension exceeds its resource limit")]
	Limit,
	#[error("Invalid extension document")]
	Invalid,
	#[error("Unsupported extension API version")]
	Version,
	#[error("Extension capability was not granted")]
	Capability,
	#[error("Invalid or unsupported WebAssembly module")]
	Module,
	#[error("Extension execution failed or exhausted its budget")]
	Execution,
	#[error("Extension returned an invalid response")]
	Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionKind {
	Plugin,
	Theme,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
	SelectedMessage,
	Composer,
	Storage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
	Message,
	Composer,
	Panel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
	pub id: String,
	pub label: String,
	pub surface: Surface,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
	pub api_version: u32,
	pub id: String,
	pub name: String,
	pub version: String,
	pub author: String,
	pub license: String,
	pub source: String,
	pub kind: ExtensionKind,
	#[serde(default)]
	pub capabilities: Vec<Capability>,
	#[serde(default)]
	pub actions: Vec<Action>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemePalette {
	#[serde(default)]
	pub colors: BTreeMap<String, String>,
	#[serde(default)]
	pub backdrop: Option<[String; 2]>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
	#[serde(default)]
	pub light: ThemePalette,
	#[serde(default)]
	pub dark: ThemePalette,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
	pub manifest: Manifest,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub theme: Option<Theme>,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub wasm: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
	pub api_version: u32,
	pub entries: Vec<CatalogEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogEntry {
	pub manifest: Manifest,
	pub release_url: String,
	pub sha256: String,
	pub download_bytes: u64,
	pub source_commit: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
	#[serde(default)]
	pub replacement: Option<String>,
	#[serde(default)]
	pub panel: Vec<Element>,
	#[serde(default)]
	pub storage: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
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

pub fn valid_id(value: &str) -> bool {
	!value.is_empty()
		&& value.len() <= 64
		&& value
			.bytes()
			.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
		&& value.as_bytes()[0].is_ascii_alphanumeric()
		&& !matches!(
			value,
			"con"
				| "prn" | "aux"
				| "nul" | "com1"
				| "com2" | "com3"
				| "com4" | "com5"
				| "com6" | "com7"
				| "com8" | "com9"
				| "lpt1" | "lpt2"
				| "lpt3" | "lpt4"
				| "lpt5" | "lpt6"
				| "lpt7" | "lpt8"
				| "lpt9"
		)
}

/// Metadata links never carry credentials and are opened only through an explicit host action.
pub fn valid_https_url(value: &str) -> bool {
	value.len() <= 2048
		&& url::Url::parse(value).is_ok_and(|url| {
			url.scheme() == "https"
				&& url.host_str().is_some()
				&& url.username().is_empty()
				&& url.password().is_none()
		})
}

impl Manifest {
	pub fn validate(&self) -> Result<(), Error> {
		if self.api_version != API_VERSION {
			return Err(Error::Version);
		}
		if !valid_id(&self.id) || !valid_https_url(&self.source) {
			return Err(Error::Invalid);
		}
		for value in [&self.name, &self.version, &self.author, &self.license] {
			if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
				return Err(Error::Invalid);
			}
		}
		if self.capabilities.len() > 3 || self.actions.len() > 16 {
			return Err(Error::Limit);
		}
		let mut capabilities = BTreeSet::new();
		if self.capabilities.iter().any(|c| !capabilities.insert(*c)) {
			return Err(Error::Invalid);
		}
		let mut ids = BTreeSet::new();
		for action in &self.actions {
			if !valid_id(&action.id)
				|| !ids.insert(&action.id)
				|| action.label.is_empty()
				|| action.label.len() > 128
				|| action.label.chars().any(char::is_control)
			{
				return Err(Error::Invalid);
			}
			let required = match action.surface {
				Surface::Message => Some(Capability::SelectedMessage),
				Surface::Composer => Some(Capability::Composer),
				Surface::Panel => None,
			};
			if required.is_some_and(|c| !capabilities.contains(&c)) {
				return Err(Error::Capability);
			}
		}
		if self.kind == ExtensionKind::Theme
			&& (!self.actions.is_empty() || !self.capabilities.is_empty())
		{
			return Err(Error::Invalid);
		}
		if self.kind == ExtensionKind::Plugin && self.actions.is_empty() {
			return Err(Error::Invalid);
		}
		Ok(())
	}
}

pub fn parse_color(value: &str) -> Result<[u8; 4], Error> {
	let bytes = value.as_bytes();
	if !matches!(bytes.len(), 7 | 9)
		|| bytes[0] != b'#'
		|| !bytes[1..].iter().all(u8::is_ascii_hexdigit)
	{
		return Err(Error::Invalid);
	}
	let mut rgba = [0, 0, 0, 255];
	for (i, pair) in bytes[1..].as_chunks::<2>().0.iter().enumerate() {
		let digit = |b: u8| {
			if b.is_ascii_digit() {
				b - b'0'
			} else {
				b.to_ascii_lowercase() - b'a' + 10
			}
		};
		rgba[i] = digit(pair[0]) * 16 + digit(pair[1]);
	}
	Ok(rgba)
}

impl Theme {
	pub fn validate(&self) -> Result<(), Error> {
		const NAMES: &[&str] = &[
			"base",
			"sidebar",
			"chat",
			"raised",
			"hover",
			"selected",
			"border",
			"text_strong",
			"text",
			"muted",
			"link",
			"accent",
			"accent_text",
			"positive",
			"warning",
			"danger",
			"mention_bg",
			"mention_text",
		];
		for palette in [&self.light, &self.dark] {
			if palette.colors.len() > NAMES.len() {
				return Err(Error::Limit);
			}
			for (name, color) in &palette.colors {
				if !NAMES.contains(&name.as_str()) {
					return Err(Error::Invalid);
				}
				parse_color(color)?;
			}
			if let Some(stops) = &palette.backdrop {
				for color in stops {
					parse_color(color)?;
				}
			}
		}
		Ok(())
	}
}

impl Package {
	pub fn validate(&self) -> Result<(), Error> {
		self.manifest.validate()?;
		match self.manifest.kind {
			ExtensionKind::Theme if self.wasm.is_empty() => {
				self.theme.as_ref().ok_or(Error::Invalid)?.validate()
			}
			ExtensionKind::Plugin if self.theme.is_none() => {
				if self.wasm.len() > MAX_MODULE_BYTES {
					return Err(Error::Limit);
				}
				runtime::validate_module(&self.wasm)
			}
			_ => Err(Error::Invalid),
		}
	}
}

pub fn parse_package(bytes: &[u8]) -> Result<Package, Error> {
	if bytes.len() > MAX_PACKAGE_BYTES {
		return Err(Error::Limit);
	}
	let package: Package = serde_json::from_slice(bytes).map_err(|_| Error::Invalid)?;
	package.validate()?;
	Ok(package)
}

pub fn parse_catalog(bytes: &[u8]) -> Result<Catalog, Error> {
	if bytes.len() > MAX_CATALOG_BYTES {
		return Err(Error::Limit);
	}
	let catalog: Catalog = serde_json::from_slice(bytes).map_err(|_| Error::Invalid)?;
	if catalog.api_version != API_VERSION {
		return Err(Error::Version);
	}
	if catalog.entries.len() > 256 {
		return Err(Error::Limit);
	}
	let mut ids = BTreeSet::new();
	for entry in &catalog.entries {
		entry.manifest.validate()?;
		if !ids.insert(&entry.manifest.id)
			|| !valid_https_url(&entry.release_url)
			|| entry.sha256.len() != 64
			|| !entry.sha256.bytes().all(|b| b.is_ascii_hexdigit())
		{
			return Err(Error::Invalid);
		}
		if entry.download_bytes == 0
			|| entry.download_bytes > MAX_PACKAGE_BYTES as u64
			|| !matches!(entry.source_commit.len(), 40 | 64)
			|| !entry.source_commit.bytes().all(|b| b.is_ascii_hexdigit())
		{
			return Err(Error::Invalid);
		}
	}
	Ok(catalog)
}

impl Invocation {
	pub fn validate(&self, manifest: &Manifest) -> Result<(), Error> {
		let action = manifest
			.actions
			.iter()
			.find(|a| a.id == self.action)
			.ok_or(Error::Invalid)?;
		for (data, capability) in [
			(&self.selected_message, Capability::SelectedMessage),
			(&self.composer, Capability::Composer),
			(&self.storage, Capability::Storage),
		] {
			if let Some(data) = data {
				if !manifest.capabilities.contains(&capability) {
					return Err(Error::Capability);
				}
				if data.len() > MAX_IO_BYTES {
					return Err(Error::Limit);
				}
			}
		}
		if self.selected_message.is_some() && action.surface != Surface::Message
			|| self.composer.is_some() && action.surface != Surface::Composer
		{
			return Err(Error::Capability);
		}
		if self.values.len() > MAX_PANEL_ELEMENTS
			|| self
				.values
				.iter()
				.any(|(id, value)| !valid_id(id) || value.len() > 4096)
		{
			return Err(Error::Limit);
		}
		Ok(())
	}
}

impl Output {
	pub fn validate(&self, manifest: &Manifest, input: &Invocation) -> Result<(), Error> {
		if self.replacement.is_some()
			&& (input.composer.is_none() || !manifest.capabilities.contains(&Capability::Composer))
		{
			return Err(Error::Capability);
		}
		if self.storage.is_some() && !manifest.capabilities.contains(&Capability::Storage) {
			return Err(Error::Capability);
		}
		if self
			.replacement
			.as_ref()
			.is_some_and(|s| s.len() > MAX_IO_BYTES)
			|| self
				.storage
				.as_ref()
				.is_some_and(|s| s.len() > MAX_STORAGE_BYTES)
		{
			return Err(Error::Limit);
		}
		let mut count = 0;
		let mut ids = BTreeSet::new();
		validate_elements(&self.panel, 0, &mut count, &mut ids, manifest)
	}
}

fn validate_elements(
	elements: &[Element],
	depth: usize,
	count: &mut usize,
	ids: &mut BTreeSet<String>,
	manifest: &Manifest,
) -> Result<(), Error> {
	if depth > 8 || elements.len() > MAX_PANEL_ELEMENTS {
		return Err(Error::Limit);
	}
	for element in elements {
		*count += 1;
		if *count > MAX_PANEL_ELEMENTS {
			return Err(Error::Limit);
		}
		let (id, label) = match element {
			Element::Text { text } => {
				if text.len() > 4096 {
					return Err(Error::Limit);
				}
				continue;
			}
			Element::Row { children } => {
				validate_elements(children, depth + 1, count, ids, manifest)?;
				continue;
			}
			Element::Button { id, label } => {
				if !manifest
					.actions
					.iter()
					.any(|a| a.id == *id && a.surface == Surface::Panel)
				{
					return Err(Error::Invalid);
				}
				(id, label)
			}
			Element::TextInput { id, label, value } => {
				if value.len() > 4096 {
					return Err(Error::Limit);
				}
				(id, label)
			}
			Element::Checkbox { id, label, .. } => (id, label),
		};
		if !valid_id(id) || !ids.insert(id.clone()) || label.is_empty() || label.len() > 128 {
			return Err(Error::Invalid);
		}
	}
	Ok(())
}
