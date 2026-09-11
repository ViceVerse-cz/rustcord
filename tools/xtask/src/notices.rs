//! Notices for the packages Cargo actually built, including host build dependencies.
use serde_json::{Value, json};
use std::{
	collections::{BTreeMap, BTreeSet},
	fs,
	io::{BufRead, BufReader, Read},
	path::{Component, Path, PathBuf},
	process::{Command, Stdio},
};

const MAX_METADATA: usize = 32 * 1024 * 1024;
const MAX_INVENTORY: usize = 1024 * 1024;
const MAX_FILE: u64 = 8 * 1024 * 1024;
const MAX_BYTES: u64 = 128 * 1024 * 1024;
const MAX_FILES: usize = 8192;
const MAX_PACKAGES: usize = 4096;

pub fn build(arguments: &[&str]) -> Result<BTreeSet<String>, String> {
	let mut child = Command::new("cargo")
		.args(arguments)
		.arg("--message-format=json-render-diagnostics")
		.stdout(Stdio::piped())
		.stderr(Stdio::inherit())
		.spawn()
		.map_err(|e| e.to_string())?;
	let result: Result<BTreeSet<String>, String> = (|| {
		let mut reader = BufReader::new(child.stdout.take().ok_or("Missing Cargo output")?);
		let mut packages = BTreeSet::new();
		let mut line = Vec::new();
		loop {
			line.clear();
			let count = reader
				.by_ref()
				.take(MAX_INVENTORY as u64 + 1)
				.read_until(b'\n', &mut line)
				.map_err(|e| e.to_string())?;
			if count == 0 {
				break;
			}
			if count > MAX_INVENTORY {
				return Err("Cargo diagnostic exceeds 1 MiB".into());
			}
			let value: Value = serde_json::from_slice(&line)
				.map_err(|e| format!("Invalid Cargo build message: {e}"))?;
			match value["reason"].as_str() {
				Some("compiler-artifact") => {
					let id = value["package_id"]
						.as_str()
						.ok_or("Missing compiled package ID")?;
					if id.len() > 4096 {
						return Err("Compiled package ID exceeds limit".into());
					}
					packages.insert(id.to_owned());
					if packages.len() > MAX_PACKAGES {
						return Err("Compiled package count exceeds limit".into());
					}
				}
				Some("compiler-message") => {
					if let Some(rendered) = value["message"]["rendered"].as_str() {
						eprint!("{rendered}");
					}
				}
				_ => {}
			}
		}
		Ok(packages)
	})();
	if result.is_err() {
		let _ = child.kill();
	}
	let status = child.wait().map_err(|e| e.to_string())?;
	let packages = result?;
	if !status.success() {
		return Err("Cargo package build failed".into());
	}
	if packages.is_empty() {
		return Err("Cargo emitted no compiler artifacts".into());
	}
	Ok(packages)
}

pub fn collect(
	repo: &Path,
	destination: &Path,
	artifacts: &BTreeSet<String>,
	voice: bool,
) -> Result<(), String> {
	let host = crate::rust_host()?;
	let mut command = Command::new("cargo");
	command.current_dir(repo).args([
		"metadata",
		"--locked",
		"--offline",
		"--format-version=1",
		"--no-default-features",
		"--filter-platform",
		&host,
	]);
	if voice {
		command.args(["--features", "serein/voice"]);
	}
	let mut child = command
		.stdout(Stdio::piped())
		.stderr(Stdio::inherit())
		.spawn()
		.map_err(|e| e.to_string())?;
	let mut bytes = Vec::new();
	let read = child
		.stdout
		.take()
		.ok_or("Missing metadata output")?
		.take(MAX_METADATA as u64 + 1)
		.read_to_end(&mut bytes);
	if read.is_err() || bytes.len() > MAX_METADATA {
		let _ = child.kill();
	}
	let status = child.wait().map_err(|e| e.to_string())?;
	read.map_err(|e| e.to_string())?;
	if bytes.len() > MAX_METADATA {
		return Err("Cargo metadata exceeds 32 MiB".into());
	}
	if !status.success() {
		return Err("Locked offline Cargo metadata failed".into());
	}
	let metadata: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
	let overrides = read_bounded(
		&repo.join("assets/licenses/dependencies/overrides.json"),
		MAX_INVENTORY as u64,
	)?;
	let overrides: Value =
		serde_json::from_slice(&overrides).map_err(|e| format!("Invalid notice overrides: {e}"))?;
	assemble(repo, destination, artifacts, &metadata, &overrides)
}

fn safe_relative(value: &str) -> Result<PathBuf, String> {
	if value.is_empty()
		|| value.len() > 1024
		|| value.contains(['\\', ':'])
		|| value
			.split('/')
			.any(|part| part.is_empty() || part == "." || part == "..")
		|| value.chars().any(char::is_control)
	{
		return Err("Notice path must be a bounded relative POSIX path".into());
	}
	let path = PathBuf::from(value);
	if !path
		.components()
		.all(|part| matches!(part, Component::Normal(_)))
	{
		return Err("Invalid notice path".into());
	}
	Ok(path)
}

fn no_links(path: &Path) -> Result<(), String> {
	let mut current = PathBuf::new();
	for part in path.components() {
		if matches!(part, Component::ParentDir) {
			return Err("Notice path contains traversal".into());
		}
		current.push(part);
		if matches!(part, Component::Prefix(_) | Component::RootDir) {
			continue;
		}
		match fs::symlink_metadata(&current) {
			Ok(metadata) if metadata.file_type().is_symlink() => {
				return Err("Notice path contains a symbolic link".into());
			}
			Ok(_) => {}
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
			Err(error) => return Err(error.to_string()),
		}
	}
	Ok(())
}

fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
	no_links(path)?;
	let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
	if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum {
		return Err("Notice file is empty, not regular, or exceeds size limit".into());
	}
	let mut bytes = Vec::new();
	fs::File::open(path)
		.map_err(|e| e.to_string())?
		.take(maximum + 1)
		.read_to_end(&mut bytes)
		.map_err(|e| e.to_string())?;
	if bytes.is_empty() || bytes.len() as u64 > maximum {
		return Err("Notice file changed beyond size limit".into());
	}
	Ok(bytes)
}

fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
	value[name]
		.as_str()
		.filter(|s| !s.is_empty() && s.len() <= 4096)
		.ok_or_else(|| format!("Missing or invalid notice metadata field: {name}"))
}

fn component(value: &str) -> Result<(), String> {
	if value.is_empty()
		|| value.len() > 128
		|| !value
			.bytes()
			.all(|b| b.is_ascii_alphanumeric() || b"-_.+".contains(&b))
		|| matches!(value, "." | "..")
	{
		return Err("Invalid package name or version".into());
	}
	Ok(())
}

fn notice_name(name: &str) -> bool {
	let name = name.to_ascii_uppercase();
	["LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT"]
		.iter()
		.any(|prefix| {
			name.strip_prefix(prefix)
				.is_some_and(|suffix| suffix.is_empty() || suffix.starts_with(['.', '-', '_']))
		})
}

fn license_name(name: &str) -> bool {
	let name = name.to_ascii_uppercase();
	["LICENSE", "LICENCE", "COPYING"]
		.iter()
		.any(|prefix| name.starts_with(prefix))
}

struct File {
	source: PathBuf,
	target: String,
}

fn assemble(
	repo: &Path,
	destination: &Path,
	artifacts: &BTreeSet<String>,
	metadata: &Value,
	overrides: &Value,
) -> Result<(), String> {
	no_links(repo)?;
	let repo = repo.canonicalize().map_err(|e| e.to_string())?;
	let destination = if destination.is_absolute() {
		// Canonicalize the existing ancestor, retaining only normal missing components.
		let mut ancestor = destination.to_owned();
		let mut missing = Vec::new();
		while !ancestor.exists() {
			missing.push(
				ancestor
					.file_name()
					.ok_or("Invalid notice destination")?
					.to_owned(),
			);
			if !ancestor.pop() {
				return Err("Invalid notice destination".into());
			}
		}
		no_links(&ancestor)?;
		let mut resolved = ancestor.canonicalize().map_err(|e| e.to_string())?;
		for part in missing.into_iter().rev() {
			resolved.push(part);
		}
		resolved
	} else {
		repo.join(destination)
	};
	no_links(&destination)?;
	// Only this named packaging subtree is owned by the collector.
	if ![
		"dist/licenses/dependencies",
		"dist/voice/licenses/dependencies",
		"dist/Serein.app/Contents/Resources/licenses/dependencies",
		"dist/voice/Serein.app/Contents/Resources/licenses/dependencies",
	]
	.iter()
	.any(|path| destination == repo.join(path))
	{
		return Err("Notice destination must be the owned text or voice package subtree".into());
	}
	if overrides["schema_version"] != 1 {
		return Err("Unsupported notice override schema".into());
	}
	let entries = overrides["packages"]
		.as_array()
		.ok_or("Missing override packages")?;
	if entries.len() > MAX_PACKAGES {
		return Err("Too many notice overrides".into());
	}
	let mut extra = BTreeMap::new();
	for entry in entries {
		let key = (field(entry, "name")?, field(entry, "version")?);
		component(key.0)?;
		component(key.1)?;
		field(entry, "source")?;
		if !entry["unresolved"].is_null() {
			field(entry, "unresolved")?;
			if entry["files"].as_array().is_none_or(Vec::is_empty)
				|| entry["source_archives"]
					.as_array()
					.is_none_or(Vec::is_empty)
			{
				return Err(format!(
					"Unresolved license record for {} {} requires supplied files and a source archive",
					key.0, key.1
				));
			}
		}
		if extra.insert(key, entry).is_some() {
			return Err(format!("Duplicate notice override: {} {}", key.0, key.1));
		}
		for kind in ["files", "source_archives"] {
			let files = entry[kind]
				.as_array()
				.ok_or_else(|| format!("Override {} {} needs {kind} array", key.0, key.1))?;
			if files.len() > MAX_FILES {
				return Err("Too many override files".into());
			}
			for file in files {
				safe_relative(file.as_str().ok_or("Invalid override file")?)?;
			}
		}
	}
	let packages = metadata["packages"]
		.as_array()
		.ok_or("Missing Cargo packages")?;
	if packages.len() > MAX_PACKAGES || artifacts.len() > MAX_PACKAGES {
		return Err("Package count exceeds limit".into());
	}
	let members = metadata["workspace_members"]
		.as_array()
		.ok_or("Missing workspace members")?
		.iter()
		.filter_map(Value::as_str)
		.collect::<BTreeSet<_>>();
	let by_id = packages
		.iter()
		.map(|p| Ok((field(p, "id")?, p)))
		.collect::<Result<BTreeMap<_, _>, String>>()?;
	let mut inventory = BTreeMap::new();
	let mut copies = BTreeMap::<String, File>::new();
	let mut missing = Vec::new();
	let mut complete = true;
	for id in artifacts {
		let package = by_id
			.get(id.as_str())
			.ok_or("Compiled package missing from locked metadata")?;
		let name = field(package, "name")?;
		let version = field(package, "version")?;
		component(name)?;
		component(version)?;
		let package_root = Path::new(field(package, "manifest_path")?)
			.parent()
			.ok_or("Invalid package manifest path")?;
		no_links(package_root)?;
		let package_root = package_root.canonicalize().map_err(|e| e.to_string())?;
		if members.contains(id.as_str()) && !package_root.starts_with(repo.join("vendor")) {
			continue;
		}
		let key = format!("{name}-{version}");
		if inventory.contains_key(&key) {
			return Err(format!(
				"Multiple dependency sources share {name} {version}; resolve notice destination collision"
			));
		}
		let source = if let Some(source) = package["source"].as_str() {
			if source.len() > 4096
				|| !(source.starts_with("registry+https://") || source.starts_with("git+https://"))
			{
				return Err(format!(
					"Unsupported dependency source for {name} {version}"
				));
			}
			source.to_owned()
		} else if package_root.starts_with(repo.join("vendor")) {
			format!(
				"path:{}",
				package_root
					.strip_prefix(&repo)
					.map_err(|e| e.to_string())?
					.to_str()
					.ok_or("Invalid vendor path")?
					.replace('\\', "/")
			)
		} else {
			return Err(format!("Unreviewed local dependency: {name} {version}"));
		};
		if let Some(entry) = extra.get(&(name, version))
			&& field(entry, "source")? != source
		{
			return Err(format!(
				"Notice override source changed for {name} {version}; review exact source"
			));
		}
		let mut files = BTreeSet::new();
		let mut archives = BTreeSet::new();
		let mut covered = false;
		let mut add = |source: PathBuf, relative: String| -> Result<String, String> {
			let target = format!("{key}/{relative}");
			safe_relative(&target)?;
			if let Some(old) = copies.get(&target) {
				if old.source != source {
					return Err(format!("Notice filename collision: {name} {version}"));
				}
			} else {
				if copies.len() >= MAX_FILES {
					return Err("Notice file count exceeds limit".into());
				}
				copies.insert(
					target.clone(),
					File {
						source,
						target: target.clone(),
					},
				);
			}
			Ok(target)
		};
		let mut entry_count = 0;
		for entry in fs::read_dir(&package_root).map_err(|e| e.to_string())? {
			entry_count += 1;
			if entry_count > MAX_FILES {
				return Err(format!(
					"Package root directory too large: {name} {version}"
				));
			}
			let entry = entry.map_err(|e| e.to_string())?;
			let filename = entry.file_name();
			let Some(filename) = filename.to_str() else {
				continue;
			};
			if !notice_name(filename) {
				continue;
			}
			if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
				continue;
			}
			files.insert(add(entry.path(), format!("notices/{filename}"))?);
			covered |= license_name(filename);
		}
		if let Some(license) = package["license_file"].as_str() {
			let path = Path::new(license);
			let relative = if path.is_absolute() {
				no_links(path)?;
				path.canonicalize()
					.map_err(|e| format!("Missing license_file for {name} {version}: {e}"))?
					.strip_prefix(&package_root)
					.map_err(|_| {
						format!(
							"External license_file for {name} {version}; add reviewed exact-version override"
						)
					})?
					.to_owned()
			} else {
				safe_relative(license)?
			};
			let relative = relative
				.to_str()
				.ok_or("Invalid license_file path")?
				.replace('\\', "/");
			safe_relative(&relative)?;
			files.insert(add(
				package_root.join(&relative),
				format!("notices/{relative}"),
			)?);
			covered = true;
		}
		if let Some(entry) = extra.get(&(name, version)) {
			for (kind, output) in [("files", &mut files), ("source_archives", &mut archives)] {
				for path in entry[kind].as_array().ok_or("Invalid override files")? {
					let path = path.as_str().ok_or("Invalid override path")?;
					let relative = safe_relative(path)?;
					output.insert(add(repo.join(relative), format!("{kind}/{path}"))?);
					covered |= kind == "files";
				}
			}
		}
		if !covered {
			missing.push(format!("{name} {version}"));
		}
		let unresolved = extra
			.get(&(name, version))
			.and_then(|entry| entry["unresolved"].as_str());
		if let Some(reason) = unresolved {
			complete = false;
			eprintln!("Incomplete upstream license evidence for {name} {version}: {reason}");
		}
		inventory.insert(key, json!({"name":name,"version":version,"license":package["license"],"source":source,"files":files,"source_archives":archives,"unresolved":unresolved}));
	}
	if !missing.is_empty() {
		return Err(format!(
			"Missing dependency license texts; add exact-version overrides for: {}",
			missing.join(", ")
		));
	}
	let inventory = serde_json::to_vec_pretty(&json!({"schema_version":1,"complete":complete,"scope":"Cargo compiler artifacts, conservatively including build dependencies and procedural macros","files":copies.keys().collect::<Vec<_>>(),"packages":inventory.into_values().collect::<Vec<_>>()})).map_err(|e| e.to_string())?;
	if inventory.len() > MAX_INVENTORY {
		return Err("Notice inventory exceeds 1 MiB".into());
	}
	// Validate the old owned tree before removing it, including nested symlinks.
	if destination.exists() {
		let mut pending = vec![destination.clone()];
		let mut count = 0;
		while let Some(path) = pending.pop() {
			count += 1;
			if count > MAX_FILES * 4 {
				return Err("Previous notice tree exceeds cleanup bound".into());
			}
			no_links(&path)?;
			if fs::metadata(&path).map_err(|e| e.to_string())?.is_dir() {
				for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
					if pending.len() >= MAX_FILES * 4 {
						return Err("Previous notice tree exceeds cleanup bound".into());
					}
					pending.push(entry.map_err(|e| e.to_string())?.path());
				}
			}
		}
		fs::remove_dir_all(&destination).map_err(|e| e.to_string())?;
	}
	fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
	let mut total = inventory.len() as u64;
	for file in copies.into_values() {
		let bytes = read_bounded(&file.source, MAX_FILE)
			.map_err(|e| format!("Cannot collect {}: {e}", file.target))?;
		total += bytes.len() as u64;
		if total > MAX_BYTES {
			return Err("Collected dependency notices exceed 128 MiB".into());
		}
		let target = destination.join(&file.target);
		fs::create_dir_all(target.parent().ok_or("Invalid notice destination")?)
			.map_err(|e| e.to_string())?;
		fs::write(target, bytes).map_err(|e| e.to_string())?;
	}
	fs::write(destination.join("inventory.json"), inventory).map_err(|e| e.to_string())?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	struct Fixture(PathBuf);
	impl Fixture {
		fn new() -> Self {
			static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
			let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
			let nonce = std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.unwrap()
				.as_nanos();
			let root = std::env::temp_dir().join(format!(
				"serein-notices-{}-{nonce}-{sequence}",
				std::process::id()
			));
			fs::create_dir(&root).unwrap();
			Self(root.canonicalize().unwrap())
		}
		fn file(&self, path: &str, bytes: &[u8]) {
			let path = self.0.join(path);
			fs::create_dir_all(path.parent().unwrap()).unwrap();
			fs::write(path, bytes).unwrap();
		}
		fn metadata(&self) -> Value {
			json!({"workspace_members":["original","vendor"],"packages":[
				{"id":"original","name":"serein","version":"0.1.0","manifest_path":self.0.join("apps/desktop/Cargo.toml"),"source":null,"license":"MIT"},
				{"id":"dependency","name":"dependency","version":"1.2.3","manifest_path":self.0.join("registry/dependency/Cargo.toml"),"source":"registry+https://github.com/rust-lang/crates.io-index","license":"MIT"},
				{"id":"vendor","name":"patched","version":"2.0.0","manifest_path":self.0.join("vendor/patched/Cargo.toml"),"source":null,"license":"MPL-2.0"}
			]})
		}
		fn destination(&self) -> PathBuf {
			self.0.join("dist/licenses/dependencies")
		}
	}
	impl Drop for Fixture {
		fn drop(&mut self) {
			fs::remove_dir_all(&self.0).unwrap();
		}
	}

	#[test]
	fn compiled_inventory_covers_vendor_and_rebuild_removes_stale_files() {
		let fixture = Fixture::new();
		fixture.file("apps/desktop/Cargo.toml", b"workspace source");
		fixture.file(
			"registry/dependency/LICENSE-MIT",
			b"actual dependency grant",
		);
		fixture.file("registry/dependency/NOTICE", b"actual attribution");
		fixture.file("vendor/patched/COPYING", b"actual vendor grant");
		fixture.file("vendor/patched/nested/COPYRIGHT", b"nested attribution");
		fixture.file("assets/source.tar.gz", b"exact source archive fixture");
		let overrides = json!({"schema_version":1,"packages":[{"name":"patched","version":"2.0.0","source":"path:vendor/patched","files":["vendor/patched/nested/COPYRIGHT"],"source_archives":["assets/source.tar.gz"]}]});
		let artifacts = ["original", "dependency", "vendor"]
			.map(str::to_owned)
			.into();
		assemble(
			&fixture.0,
			&fixture.destination(),
			&artifacts,
			&fixture.metadata(),
			&overrides,
		)
		.unwrap();
		let first = fs::read(fixture.destination().join("inventory.json")).unwrap();
		let value: Value = serde_json::from_slice(&first).unwrap();
		assert_eq!(value["packages"].as_array().unwrap().len(), 2);
		assert_eq!(value["files"].as_array().unwrap().len(), 5);
		assert_eq!(value["complete"], true);
		assert!(
			!String::from_utf8(first.clone())
				.unwrap()
				.contains(fixture.0.to_str().unwrap())
		);
		fixture.file(
			"dist/licenses/dependencies/unrelated-stale.txt",
			b"previous package data",
		);
		assemble(
			&fixture.0,
			&fixture.destination(),
			&artifacts,
			&fixture.metadata(),
			&overrides,
		)
		.unwrap();
		assert!(!fixture.destination().join("unrelated-stale.txt").exists());
		assert_eq!(
			fs::read(fixture.destination().join("inventory.json")).unwrap(),
			first
		);
		assemble(
			&fixture.0,
			&fixture.destination(),
			&["dependency".to_owned()].into(),
			&fixture.metadata(),
			&overrides,
		)
		.unwrap();
		assert!(!fixture.destination().join("patched-2.0.0").exists());
		assert!(fixture.0.join("vendor/patched/COPYING").exists());
	}

	#[test]
	fn missing_coverage_and_stale_overrides_fail_but_reviewed_gaps_are_explicit() {
		let fixture = Fixture::new();
		fixture.file(
			"registry/dependency/NOTICE",
			b"Attribution alone is not a license grant",
		);
		let artifacts = ["dependency".to_owned()].into();
		let mut overrides = json!({"schema_version":1,"packages":[]});
		let error = assemble(
			&fixture.0,
			&fixture.destination(),
			&artifacts,
			&fixture.metadata(),
			&overrides,
		)
		.unwrap_err();
		assert!(error.contains("dependency 1.2.3"));
		assert!(!fixture.destination().exists());
		fixture.file(
			"assets/reference.txt",
			b"Reviewed reference, not a fabricated copyright",
		);
		fixture.file("assets/release.tar.gz", b"Original release fixture");
		overrides["packages"] = json!([{"name":"dependency","version":"1.2.3","source":"registry+https://github.com/rust-lang/crates.io-index","files":["assets/reference.txt"],"source_archives":["assets/release.tar.gz"],"unresolved":"Original release omits its license grant"}]);
		assemble(
			&fixture.0,
			&fixture.destination(),
			&artifacts,
			&fixture.metadata(),
			&overrides,
		)
		.unwrap();
		let value: Value = serde_json::from_slice(
			&fs::read(fixture.destination().join("inventory.json")).unwrap(),
		)
		.unwrap();
		assert_eq!(value["complete"], false);
		assert_eq!(
			value["packages"][0]["unresolved"],
			overrides["packages"][0]["unresolved"]
		);
		overrides["packages"][0]["source"] =
			json!("git+https://example.com/replacement#othercommit");
		assert!(
			assemble(
				&fixture.0,
				&fixture.destination(),
				&artifacts,
				&fixture.metadata(),
				&overrides
			)
			.unwrap_err()
			.contains("source changed")
		);
	}

	#[test]
	fn rejects_unsafe_paths_and_oversized_files_without_copying_outside_owned_tree() {
		let fixture = Fixture::new();
		fixture.file("registry/dependency/LICENSE", b"grant");
		let artifacts = ["dependency".to_owned()].into();
		let mut overrides = json!({"schema_version":1,"packages":[{"name":"dependency","version":"1.2.3","source":"registry+https://github.com/rust-lang/crates.io-index","files":[],"source_archives":[]}]});
		for path in [
			"../outside",
			"/absolute",
			"C:/absolute",
			"assets/../secret",
			"assets\\secret",
			"assets//secret",
			"assets/./secret",
		] {
			overrides["packages"][0]["files"] = json!([path]);
			assert!(
				assemble(
					&fixture.0,
					&fixture.destination(),
					&artifacts,
					&fixture.metadata(),
					&overrides
				)
				.is_err(),
				"{path}"
			);
		}
		overrides["packages"][0]["files"] = json!([]);
		assert!(
			assemble(
				&fixture.0,
				&fixture.0.join("assets/dependencies"),
				&artifacts,
				&fixture.metadata(),
				&overrides
			)
			.is_err()
		);
		fs::OpenOptions::new()
			.write(true)
			.open(fixture.0.join("registry/dependency/LICENSE"))
			.unwrap()
			.set_len(MAX_FILE + 1)
			.unwrap();
		assert!(
			assemble(
				&fixture.0,
				&fixture.destination(),
				&artifacts,
				&fixture.metadata(),
				&overrides
			)
			.unwrap_err()
			.contains("size limit")
		);
		assert!(!fixture.destination().join("inventory.json").exists());
	}

	#[cfg(unix)]
	#[test]
	fn refuses_source_and_previous_output_symlinks() {
		use std::os::unix::fs::symlink;
		let fixture = Fixture::new();
		fixture.file("registry/dependency/real", b"grant");
		symlink("real", fixture.0.join("registry/dependency/LICENSE")).unwrap();
		let artifacts = ["dependency".to_owned()].into();
		let overrides = json!({"schema_version":1,"packages":[]});
		assert!(
			assemble(
				&fixture.0,
				&fixture.destination(),
				&artifacts,
				&fixture.metadata(),
				&overrides
			)
			.unwrap_err()
			.contains("symbolic link")
		);
		fs::remove_file(fixture.0.join("registry/dependency/LICENSE")).unwrap();
		fixture.file("registry/dependency/LICENSE", b"grant");
		symlink(
			fixture.0.join("registry"),
			fixture.destination().join("escape"),
		)
		.unwrap();
		assert!(
			assemble(
				&fixture.0,
				&fixture.destination(),
				&artifacts,
				&fixture.metadata(),
				&overrides
			)
			.unwrap_err()
			.contains("symbolic link")
		);
		assert!(fixture.0.join("registry/dependency/LICENSE").exists());
	}
}
