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
