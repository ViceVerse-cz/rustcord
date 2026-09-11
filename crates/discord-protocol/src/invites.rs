//! Bounded public invite metadata; normal-user compatibility remains unverified.
use serde::Deserialize;
#[derive(Deserialize)]
struct Invite {
	guild: Option<Guild>,
	approximate_presence_count: Option<u64>,
	approximate_member_count: Option<u64>,
}
#[derive(Deserialize)]
struct Guild {
	id: model::Id,
	name: String,
	icon: Option<String>,
	banner: Option<String>,
}
pub fn decode(bytes: &[u8]) -> Result<model::InvitePreview, crate::DecodeError> {
	if bytes.len() > 64 * 1024 {
		return Err(crate::DecodeError);
	}
	let invite: Invite = serde_json::from_slice(bytes).map_err(|_| crate::DecodeError)?;
	let guild = invite.guild.ok_or(crate::DecodeError)?;
	if guild.id.0 == 0 || guild.name.len() > 400 {
		return Err(crate::DecodeError);
	}
	let media = |hash: Option<String>, folder: &str| {
		hash.filter(|h| model::valid_avatar_hash(h))
			.map(|hash| model::EmbedMedia {
				url: Some(format!(
					"https://cdn.discordapp.com/{folder}/{}/{hash}.png?size=512",
					guild.id
				)),
				width: if folder == "icons" { 128 } else { 512 },
				height: if folder == "icons" { 128 } else { 288 },
				..Default::default()
			})
	};
	let counts = match (
		invite.approximate_presence_count,
		invite.approximate_member_count,
	) {
		(Some(online), Some(members)) => Some(format!("● {online} online · {members} members")),
		(_, Some(members)) => Some(format!("{members} members")),
		_ => None,
	};
	Ok(model::InvitePreview {
		guild: guild.id,
		embed: model::Embed {
			kind: "discord-invite".into(),
			title: Some(guild.name),
			description: counts,
			thumbnail: media(guild.icon, "icons"),
			image: media(guild.banner, "banners"),
			..Default::default()
		},
	})
}
