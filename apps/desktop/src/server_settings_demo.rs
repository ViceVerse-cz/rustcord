//! Explicit offline fixture; never loads a saved session or changes a real guild.
use client_core::{Command, Envelope, Event, State};
use model::{
	Id,
	server_settings::{Edit, Settings, Trait},
};

fn snapshot(state: &State, guild: Id) -> Settings {
	let server = state
		.guilds
		.iter()
		.find(|server| server.id == guild)
		.expect("fixture guild");
	Settings {
		guild,
		name: server.name.clone(),
		icon: server.icon.clone(),
		banner_color: Some(0x245ae8),
		traits: vec![
			Trait {
				label: "Games".into(),
				emoji: Some("🎮".into()),
			},
			Trait {
				label: "Community".into(),
				emoji: None,
			},
		],
		description: "A synthetic workspace for friends, games and conversation.".into(),
		online_count: Some(4),
		member_count: Some(9),
		system_channel_id: state
			.channels
			.iter()
			.find(|channel| channel.guild == Some(guild) && channel.kind == 0)
			.map(|channel| channel.id),
		default_message_notifications: 1,
		activity_feed: Some(true),
		features: vec![
			"COMMUNITY".into(),
			model::server_settings::ACTIVITY_ENABLED.into(),
		],
		..Default::default()
	}
}

pub fn execute(state: &State, guild: Id, request: u64, edit: Option<Box<Edit>>) -> Event {
	let mut value = state
		.server_settings
		.snapshot
		.as_ref()
		.filter(|value| value.guild == guild)
		.cloned()
		.unwrap_or_else(|| snapshot(state, guild));
	if let Some(edit) = edit {
		edit.apply(&mut value);
		if matches!(edit.icon, model::Patch::Value(_)) {
			value.icon = Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into());
		}
	}
	assert!(value.valid(), "synthetic server settings remain valid");
	Event::ServerSettings(client_core::server_settings::Event {
		guild,
		request,
		result: Ok(Box::new(value)),
		refreshed: None,
	})
}

pub fn execute_admin(
	state: &State,
	guild: Id,
	request: u64,
	action: model::server_admin::Action,
) -> Event {
	use model::server_admin::{Action, Emoji, Emojis, Member, Members, Result as Outcome, Role};
	assert!(
		action.valid(),
		"synthetic admin action must satisfy wire bounds"
	);
	let owner = state.user.as_ref().expect("fixture user");
	let result = match action {
		Action::LoadEmojis
		| Action::CreateEmoji { .. }
		| Action::RenameEmoji { .. }
		| Action::DeleteEmoji { .. } => {
			let mut page = state.server_admin.emojis.clone().unwrap_or_else(|| Emojis {
				items: vec![Emoji {
					emoji: model::CustomEmoji {
						id: Id(9001),
						name: "serein".into(),
						animated: false,
						available: true,
						managed: false,
						roles: Some(Vec::new()),
					},
					uploader: Some(owner.clone()),
				}],
				static_limit: Some(50),
				animated_limit: Some(50),
			});
			match action {
				Action::CreateEmoji { name, image } => {
					let id = Id(page
						.items
						.iter()
						.map(|row| row.emoji.id.0)
						.max()
						.unwrap_or(9000) + 1);
					page.items.push(Emoji {
						emoji: model::CustomEmoji {
							id,
							name,
							animated: image.starts_with("data:image/gif"),
							available: true,
							managed: false,
							roles: Some(Vec::new()),
						},
						uploader: Some(owner.clone()),
					});
				}
				Action::RenameEmoji { id, name } => {
					if let Some(row) = page.items.iter_mut().find(|row| row.emoji.id == id) {
						row.emoji.name = name;
					}
				}
				Action::DeleteEmoji { id } => page.items.retain(|row| row.emoji.id != id),
				_ => {}
			}
			Outcome::Emojis(page)
		}
		Action::LoadMembers(query) => {
			let mut page = state.server_admin.members.clone().unwrap_or_else(|| {
				let roles: Vec<_> = state
					.permissions
					.guilds
					.get(&guild)
					.and_then(|guild| guild.roles.as_ref())
					.into_iter()
					.flatten()
					.map(|role| Role {
						role: role.clone(),
						managed: false,
					})
					.collect();
				let items = [
					"Avery", "Mika", "Rowan", "Sam", "Taylor", "Morgan", "Alex", "Jamie",
				]
				.into_iter()
				.enumerate()
				.map(|(index, name)| {
					let mut user = owner.clone();
					user.id = Id(((1_620_070_400_000u64 + index as u64 * 86_400_000
						- 1_420_070_400_000)
						<< 22) | 1);
					user.name = name.into();
					user.avatar = None;
					Member {
						user,
						nick: None,
						roles: roles
							.iter()
							.filter(|role| role.role.id != guild)
							.take(index % 2)
							.map(|role| role.role.id)
							.collect(),
						joined_at: Some(1_789_200_000_000 - index as i128 * 86_400_000),
						join_source: Some(1),
						invite_code: Some("demo".into()),
						flags: Some(0),
						unusual_dm_until: None,
						timeout_until: None,
					}
				})
				.collect::<Vec<_>>();
				Members {
					total: items.len() as u64,
					items,
					roles,
					next: None,
					features: Vec::new(),
					show_in_channel_list: Some(false),
				}
			});
			page.items.retain(|row| {
				query.search.is_empty()
					|| row
						.user
						.name
						.to_lowercase()
						.contains(&query.search.to_lowercase())
					|| row.user.id.to_string() == query.search
			});
			match query.sort {
				2 => page.items.sort_by_key(|row| row.joined_at),
				3 => page.items.sort_by_key(|row| std::cmp::Reverse(row.user.id)),
				4 => page.items.sort_by_key(|row| row.user.id),
				_ => page
					.items
					.sort_by_key(|row| std::cmp::Reverse(row.joined_at)),
			}
			Outcome::Members(page)
		}
		Action::SetRole { user, .. } | Action::SetNickname { user, .. } => {
			let mut member = state
				.server_admin
				.members
				.as_ref()
				.and_then(|page| page.items.iter().find(|row| row.user.id == user))
				.expect("fixture target member")
				.clone();
			match action {
				Action::SetRole { role, assigned, .. } => {
					member.roles.retain(|id| *id != role);
					if assigned {
						member.roles.push(role);
					}
				}
				Action::SetNickname { nick, .. } => {
					member.nick = (!nick.is_empty()).then_some(nick)
				}
				_ => {}
			}
			Outcome::Member(member)
		}
		Action::Kick { user } => Outcome::Kicked(user),
		Action::Prune { .. } => Outcome::Pruned(Some(0)),
		Action::ShowMembers { enabled } => Outcome::ChannelList(enabled),
	};
	assert!(
		result.valid(),
		"synthetic administration response must stay bounded"
	);
	Event::ServerAdmin(client_core::server_admin::Event {
		guild,
		request,
		result: Ok(result),
	})
}

pub fn open(state: &mut State, messaging: &mut ui::MessagingUi) {
	let guild = state.guilds[0].id;
	let user = state.user.as_ref().expect("fixture user").id;
	let mut permissions = test_support::permission_snapshot(state);
	let permission = permissions
		.guilds
		.iter_mut()
		.find(|permission| permission.id == guild)
		.unwrap();
	permission.owner = Some(user);
	let scenario = std::env::args().find_map(|arg| {
		arg.strip_prefix("--demo-server-permission=")
			.map(str::to_owned)
	});
	if let Some(scenario) = scenario.as_deref() {
		permission.owner = Some(Id(u64::MAX));
		if let Some(roles) = &mut permission.roles {
			roles[0].bits |= match scenario {
				"admin" => model::permissions::ADMINISTRATOR,
				"manager" => model::permissions::MANAGE_GUILD,
				_ => 0,
			};
		}
		if scenario == "unknown" {
			permission.roles = None;
		}
	}
	state.apply(Envelope {
		generation: state.generation,
		event: Event::Permissions(client_core::permissions::Event::Snapshot(permissions)),
	});
	if let Some(page) =
		std::env::args().find_map(|arg| arg.strip_prefix("--demo-server-page=").map(str::to_owned))
	{
		if let Some(Command::ServerAdmin {
			guild,
			request,
			action,
		}) = messaging.preview_server_admin(state, guild, &page)
		{
			let event = execute_admin(state, guild, request, *action);
			state.apply(Envelope {
				generation: state.generation,
				event,
			});
		}
		return;
	}
	if let Some(Command::ServerSettings {
		guild,
		request,
		edit,
	}) = state.load_server_settings(guild)
	{
		let event = execute(state, guild, request, edit);
		state.apply(Envelope {
			generation: state.generation,
			event,
		});
	}
	if let Some(Command::ServerSettings {
		guild,
		request,
		edit,
	}) = messaging.preview_server_settings(state, guild)
	{
		let event = execute(state, guild, request, edit);
		state.apply(Envelope {
			generation: state.generation,
			event,
		});
	}
}
