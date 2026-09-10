use crate::Patch;

/// Presence only: unrendered service payloads are never retained here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ExtraContent {
	pub poll: bool,
	pub sticker_items: bool,
	pub stickers: bool,
	pub components: bool,
	pub components_v2: bool,
}
impl ExtraContent {
	pub fn any(&self) -> bool {
		self.bits() != 0
	}
	pub fn bits(&self) -> u8 {
		u8::from(self.poll)
			| (u8::from(self.sticker_items) << 1)
			| (u8::from(self.stickers) << 2)
			| (u8::from(self.components) << 3)
			| (u8::from(self.components_v2) << 4)
	}
	pub fn from_bits(bits: u8) -> Option<Self> {
		(bits & !31 == 0).then_some(Self {
			poll: bits & 1 != 0,
			sticker_items: bits & 2 != 0,
			stickers: bits & 4 != 0,
			components: bits & 8 != 0,
			components_v2: bits & 16 != 0,
		})
	}
}

#[derive(Clone, Debug, Default)]
pub struct ExtraContentPatch {
	pub poll: Patch<bool>,
	pub sticker_items: Patch<bool>,
	pub stickers: Patch<bool>,
	pub components: Patch<bool>,
	pub components_v2: Patch<bool>,
}
impl ExtraContentPatch {
	pub fn apply(&self, content: &mut ExtraContent) {
		for (patch, value) in [
			(&self.poll, &mut content.poll),
			(&self.sticker_items, &mut content.sticker_items),
			(&self.stickers, &mut content.stickers),
			(&self.components, &mut content.components),
			(&self.components_v2, &mut content.components_v2),
		] {
			match patch {
				Patch::Absent => {}
				Patch::Null => *value = false,
				Patch::Value(new) => *value = *new,
			}
		}
	}
	pub fn merge(&mut self, newer: &Self) {
		for (old, new) in [
			(&mut self.poll, &newer.poll),
			(&mut self.sticker_items, &newer.sticker_items),
			(&mut self.stickers, &newer.stickers),
			(&mut self.components, &newer.components),
			(&mut self.components_v2, &newer.components_v2),
		] {
			if !matches!(new, Patch::Absent) {
				old.clone_from(new);
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn marker_bits_round_trip_and_independent_patches_merge() {
		for bits in 0..=u8::MAX {
			let content = ExtraContent::from_bits(bits);
			if bits <= 31 {
				let content = content.unwrap();
				assert_eq!(content.bits(), bits);
				assert_eq!(content.any(), bits != 0);
			} else {
				assert!(content.is_none());
			}
		}
		let mut first = ExtraContentPatch {
			poll: Patch::Value(true),
			sticker_items: Patch::Value(true),
			components: Patch::Value(true),
			..Default::default()
		};
		first.merge(&ExtraContentPatch {
			poll: Patch::Null,
			sticker_items: Patch::Value(false),
			stickers: Patch::Value(true),
			..Default::default()
		});
		first.merge(&ExtraContentPatch::default());
		let mut content = ExtraContent::from_bits(16).unwrap();
		first.apply(&mut content);
		assert_eq!(content.bits(), 4 | 8 | 16);
	}
}
