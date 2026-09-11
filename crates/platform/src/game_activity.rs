//! Bounded executable-name matching. Call only on a worker while activity sharing is enabled.
//! Process names are neither retained nor logged; only a recognized game's static title escapes.

#[cfg(any(target_os = "windows", target_os = "linux"))]
const MAX_PROCESSES: usize = 4096;

#[cfg(any(target_os = "windows", target_os = "linux", test))]
fn game_name(executable: &str) -> Option<&'static str> {
	// ponytail: exact allowlist misses renamed/unknown games; add verified basenames when needed.
	[
		("osu!.exe", "osu!"),
		("osu.exe", "osu!"),
		("osu!", "osu!"),
		("osu", "osu!"),
		("cs2.exe", "Counter-Strike 2"),
		("cs2", "Counter-Strike 2"),
		("dota2.exe", "Dota 2"),
		("dota2", "Dota 2"),
		("Terraria.exe", "Terraria"),
		("Terraria", "Terraria"),
		("Stardew Valley.exe", "Stardew Valley"),
		("StardewValley", "Stardew Valley"),
	]
	.into_iter()
	.find_map(|(name, title)| executable.eq_ignore_ascii_case(name).then_some(title))
}

/// Returns one recognized running game, or an error that must clear any previous activity.
/// Matching uses executable basenames only, never process memory, paths or window titles.
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
pub fn detect_game() -> Result<Option<&'static str>, &'static str> {
	use windows::{
		Win32::{
			Foundation::{CloseHandle, ERROR_NO_MORE_FILES, HANDLE},
			System::Diagnostics::ToolHelp::{
				CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
				TH32CS_SNAPPROCESS,
			},
		},
		core::HRESULT,
	};

	struct Snapshot(HANDLE);
	impl Drop for Snapshot {
		fn drop(&mut self) {
			// SAFETY: this owns the valid snapshot handle, never cloned or closed elsewhere.
			let _ = unsafe { CloseHandle(self.0) };
		}
	}
	// SAFETY: documented process-only flags; no pointers and no target process opened.
	let snapshot = Snapshot(
		unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
			.map_err(|_| "Game detection could not inspect running processes.")?,
	);
	let mut entry = PROCESSENTRY32W {
		dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
		..Default::default()
	};
	// SAFETY: a valid live snapshot and writable initialized entry with its required size.
	let mut next = unsafe { Process32FirstW(snapshot.0, &mut entry) };
	let mut detected = None;
	for _ in 0..MAX_PROCESSES {
		if let Err(error) = next {
			return if error.code() == HRESULT::from_win32(ERROR_NO_MORE_FILES.0) {
				Ok(detected)
			} else {
				Err("Game detection could not inspect running processes.")
			};
		}
		let length = entry
			.szExeFile
			.iter()
			.position(|&character| character == 0)
			.unwrap_or(entry.szExeFile.len());
		if let Ok(name) = String::from_utf16(&entry.szExeFile[..length])
			&& let Some(game) = game_name(&name)
		{
			// Stable choice when several supported games run at once, regardless of OS order.
			detected = Some(detected.map_or(game, |previous: &'static str| previous.min(game)));
		}
		// SAFETY: the snapshot and entry remain valid throughout this synchronous iteration.
		next = unsafe { Process32NextW(snapshot.0, &mut entry) };
	}
	if next.is_err_and(|error| error.code() == HRESULT::from_win32(ERROR_NO_MORE_FILES.0)) {
		Ok(detected)
	} else {
		Err("Game detection exceeded its process limit or could not finish the scan.")
	}
}

#[cfg(target_os = "linux")]
pub fn detect_game() -> Result<Option<&'static str>, &'static str> {
	use std::{fs, io::Read};
	let processes =
		fs::read_dir("/proc").map_err(|_| "Game detection could not inspect running processes.")?;
	let mut detected = None;
	for (index, process) in processes.enumerate() {
		if index >= MAX_PROCESSES {
			return Err("Game detection exceeded its process limit.");
		}
		let process = process.map_err(|_| "Game detection could not read a process entry.")?;
		if !process
			.file_name()
			.as_encoded_bytes()
			.iter()
			.all(u8::is_ascii_digit)
		{
			continue;
		}
		let mut file = match fs::File::open(process.path().join("comm")) {
			Ok(file) => file,
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
			Err(_) => return Err("Game detection could not read a process name."),
		};
		let mut buffer = [0_u8; 128];
		let mut length = 0;
		while length < buffer.len() {
			let read = file
				.read(&mut buffer[length..])
				.map_err(|_| "Game detection could not read a process name.")?;
			if read == 0 {
				break;
			}
			length += read;
		}
		if length < buffer.len()
			&& let Ok(name) = std::str::from_utf8(&buffer[..length])
			&& let Some(game) = game_name(name.strip_suffix('\n').unwrap_or(name))
		{
			detected = Some(detected.map_or(game, |previous: &'static str| previous.min(game)));
		}
	}
	Ok(detected)
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub fn detect_game() -> Result<Option<&'static str>, &'static str> {
	Err("Automatic game detection is not available on this platform.")
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn recognizes_exact_game_aliases_only() {
		for alias in ["osu!.exe", "osu.exe", "OSU!.EXE", "osu!", "osu"] {
			assert_eq!(game_name(alias), Some("osu!"));
		}
		assert_eq!(game_name("cs2.exe"), Some("Counter-Strike 2"));
		for name in [
			"",
			"not-osu.exe",
			"osu!.exe.bak",
			"osu!installer.exe",
			"java",
			"javaw.exe",
			"C:\\Games\\osu!.exe",
			"/games/osu",
			"cs2-helper.exe",
			"osu!.exe\0",
		] {
			assert_eq!(game_name(name), None);
		}
	}
}
