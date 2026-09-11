use local_store::AppPreferences;

#[derive(Default)]
pub struct Settings {
	pub current: AppPreferences,
	pub state: crate::toggle_setting::Settings,
}
impl Settings {
	pub fn observe(&mut self, ui: &ui::MessagingUi) {
		let value = AppPreferences {
			notifications_enabled: ui.notifications_enabled,
			show_hidden_channels: ui.show_hidden_channels,
			voice_noise_suppression: ui.voice_noise_suppression,
			voice_push_to_talk: ui.voice_push_to_talk,
			voice_input: ui.voice_input.clone(),
			voice_output: ui.voice_output.clone(),
			input_percent: ui.voice_gain.input_percent,
			output_percent: ui.voice_gain.output_percent,
		};
		if value != self.current {
			self.state.touched = true;
			self.state.failed = !value.is_valid();
			if value.is_valid() {
				self.current = value;
				self.state.dirty = true;
			}
		}
	}
	pub fn apply(&self, ui: &mut ui::MessagingUi) {
		let value = &self.current;
		ui.notifications_enabled = value.notifications_enabled;
		ui.show_hidden_channels = value.show_hidden_channels;
		ui.voice_noise_suppression = value.voice_noise_suppression;
		ui.voice_push_to_talk = value.voice_push_to_talk;
		ui.voice_input.clone_from(&value.voice_input);
		ui.voice_output.clone_from(&value.voice_output);
		ui.voice_gain.input_percent = value.input_percent;
		ui.voice_gain.output_percent = value.output_percent;
	}
}
