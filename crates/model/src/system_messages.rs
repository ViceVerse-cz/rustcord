use crate::Message;

// Public message type IDs: https://docs.discord.com/developers/resources/message#message-types
// Do not infer missing call outcomes, subscription details, or thread contents.
pub(super) fn summary(message: &Message) -> Option<String> {
	if matches!(message.kind, 0 | 19 | 20 | 23) {
		return None;
	}
	let actor: String = message.author.name.chars().take(80).collect();
	let target = || {
		message.mentions.first().map_or_else(
			|| "a member".into(),
			|user| user.name.chars().take(80).collect::<String>(),
		)
	};
	Some(match message.kind {
		1 => format!("{actor} added {} to the conversation.", target()),
		2 if message
			.mentions
			.first()
			.is_some_and(|u| u.id == message.author.id) =>
		{
			format!("{actor} left the conversation.")
		}
		2 => format!("{actor} removed {} from the conversation.", target()),
		3 => format!("Call · {actor}"),
		4 => format!("{actor} changed the channel name."),
		5 => format!("{actor} changed the channel icon."),
		6 => format!("{actor} pinned a message to this channel."),
		7 => format!("Welcome, {actor}! Joined the server."),
		8 => format!("{actor} boosted the server!"),
		9..=11 => format!(
			"{actor} boosted the server! The server reached level {}.",
			message.kind - 8
		),
		12 => format!("{actor} added a channel follow."),
		14 => "This server is no longer eligible for Server Discovery.".into(),
		15 => "This server is eligible for Server Discovery again.".into(),
		16 => "Server Discovery eligibility warning.".into(),
		17 => "Final Server Discovery eligibility warning.".into(),
		18 => format!("{actor} created a thread."),
		21 => "Thread started from an earlier message.".into(),
		22 => "Invite reminder · Invite people to this server.".into(),
		24 => "AutoMod action.".into(),
		25 => format!("{actor} purchased or renewed a role subscription."),
		26 => "Premium subscription offer.".into(),
		27 => format!("{actor} started a Stage."),
		28 => "The Stage ended.".into(),
		29 => format!("{actor} became a Stage speaker."),
		31 => format!("{actor} changed the Stage topic."),
		32 => "Server application premium subscription.".into(),
		36 => "Server alert mode enabled.".into(),
		37 => "Server alert mode disabled.".into(),
		38 => "A server raid was reported.".into(),
		39 => "A server incident was reported as a false alarm.".into(),
		44 => "Purchase notification.".into(),
		46 => "Poll results.".into(),
		_ => return None,
	})
}
