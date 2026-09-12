//! Documented role routes, with unofficial normal-user member search filters.
use crate::{DecodeError, permissions::List};
use model::{
	Id, Patch,
	server_roles::{Catalog, Colors, Edit, Role},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
#[derive(Deserialize)]
struct ColorsWire {
	primary_color: u32,
	secondary_color: Option<u32>,
	tertiary_color: Option<u32>,
}
#[derive(Deserialize)]
struct RoleWire {
	id: Id,
	name: String,
	#[serde(deserialize_with = "crate::permissions::bits")]
	permissions: u128,
	#[serde(default)]
	color: u32,
	colors: Option<ColorsWire>,
	position: i32,
	hoist: bool,
	mentionable: bool,
	managed: bool,
	icon: Option<String>,
	unicode_emoji: Option<String>,
}
impl RoleWire {
	fn checked(self) -> Result<Role, DecodeError> {
		let colors = self.colors.map_or(
			Colors {
				primary: self.color,
				..Colors::default()
			},
			|colors| Colors {
				primary: colors.primary_color,
				secondary: colors.secondary_color,
				tertiary: colors.tertiary_color,
			},
		);
		let role = Role {
			id: self.id,
			name: self.name,
			colors,
			permissions: self.permissions,
			position: self.position,
			hoist: self.hoist,
			mentionable: self.mentionable,
			managed: self.managed,
			icon: self.icon,
			unicode_emoji: self.unicode_emoji,
			member_count: None,
		};
		if !role.valid() {
			return Err(DecodeError);
		}
		Ok(role)
	}
}
pub fn role(bytes: &[u8]) -> Result<Role, DecodeError> {
	crate::decode::<RoleWire>(bytes)?.checked()
}
pub fn catalog(bytes: &[u8], guild: Id) -> Result<Catalog, DecodeError> {
	#[derive(Deserialize)]
	struct Guild {
		id: Id,
		roles: List<RoleWire, 512>,
		features: List<String, 256>,
	}
	let guild_metadata: Guild = crate::decode(bytes)?;
	if guild_metadata.id != guild {
		return Err(DecodeError);
	}
	let mut catalog = Catalog {
		guild,
		items: guild_metadata
			.roles
			.0
			.into_iter()
			.map(RoleWire::checked)
			.collect::<Result<_, _>>()?,
		features: guild_metadata.features.0,
	};
	catalog
		.items
		.sort_by(|a, b| b.position.cmp(&a.position).then_with(|| a.id.cmp(&b.id)));
	if !catalog.valid() {
		return Err(DecodeError);
	}
	Ok(catalog)
}
pub fn counts(bytes: &[u8], catalog: &mut Catalog) -> Result<(), DecodeError> {
	if bytes.len() > 64 * 1024 {
		return Err(DecodeError);
	}
	let counts: std::collections::BTreeMap<Id, u64> = crate::decode(bytes)?;
	if counts.len() > model::server_roles::MAX_ROLES {
		return Err(DecodeError);
	}
	for role in &mut catalog.items {
		role.member_count = counts.get(&role.id).copied();
	}
	Ok(())
}
pub fn encode_edit(edit: &Edit, existing: Option<&Role>) -> Result<Value, DecodeError> {
	if !edit.valid() {
		return Err(DecodeError);
	}
	let mut body = Map::new();
	if let Some(name) = &edit.name {
		body.insert("name".into(), json!(name));
	}
	if let Some(colors) = edit.colors {
		body.insert("colors".into(), json!({"primary_color":colors.primary,"secondary_color":colors.secondary,"tertiary_color":colors.tertiary}));
	}
	if let Some(bits) = edit.permissions {
		let bits = existing.map_or(bits, |role| {
			(role.permissions & !edit.permission_mask) | (bits & edit.permission_mask)
		});
		body.insert("permissions".into(), json!(bits.to_string()));
	}
	if let Some(value) = edit.hoist {
		body.insert("hoist".into(), json!(value));
	}
	if let Some(value) = edit.mentionable {
		body.insert("mentionable".into(), json!(value));
	}
	match &edit.icon {
		Patch::Value(uri) => {
			if !crate::group_actions::valid_icon_data_uri(uri) {
				return Err(DecodeError);
			}
			body.insert("icon".into(), json!(uri));
			body.insert("unicode_emoji".into(), Value::Null);
		}
		Patch::Null => {
			body.insert("icon".into(), Value::Null);
		}
		Patch::Absent => {}
	}
	match &edit.unicode_emoji {
		Patch::Value(emoji) => {
			body.insert("unicode_emoji".into(), json!(emoji));
			body.insert("icon".into(), Value::Null);
		}
		Patch::Null => {
			body.insert("unicode_emoji".into(), Value::Null);
		}
		Patch::Absent => {}
	}
	Ok(Value::Object(body))
}
pub fn matches_edit(edit: &Edit, saved: &Role, creating: bool) -> bool {
	let mut expected = saved.clone();
	edit.apply(&mut expected);
	if creating && let Some(bits) = edit.permissions {
		expected.permissions = bits;
	}
	let icon_matches = match &edit.icon {
		Patch::Value(_) => saved.icon.is_some(),
		Patch::Null => saved.icon.is_none(),
		Patch::Absent => true,
	};
	icon_matches && expected == *saved
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn role_edit_preserves_unknown_bits_and_omits_unchanged_colors() {
		let role = Role {
			id: Id(4),
			permissions: (1 << 110) | 1024,
			colors: Colors {
				primary: 1,
				secondary: Some(2),
				tertiary: None,
			},
			..Role::default()
		};
		let edit = Edit {
			permissions: Some(2048),
			permission_mask: 1024 | 2048,
			name: Some("Renamed".into()),
			..Edit::default()
		};
		let body = encode_edit(&edit, Some(&role)).unwrap();
		assert_eq!(body["permissions"], ((1_u128 << 110) | 2048).to_string());
		assert!(body.get("colors").is_none());
		assert!(body.get("icon").is_none());
		let clear = encode_edit(
			&Edit {
				icon: Patch::Null,
				..Edit::default()
			},
			Some(&role),
		)
		.unwrap();
		assert_eq!(clear["icon"], Value::Null);
		assert!(
			encode_edit(
				&Edit {
					colors: Some(Colors {
						primary: 1,
						secondary: Some(2),
						tertiary: Some(3)
					}),
					..Edit::default()
				},
				Some(&role)
			)
			.is_err()
		);
		assert!(
			encode_edit(
				&Edit {
					icon: Patch::Value("data:image/png;base64,bad".into()),
					..Edit::default()
				},
				Some(&role)
			)
			.is_err()
		);
	}
}
