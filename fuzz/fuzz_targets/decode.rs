#![no_main]

use discord_protocol::{ChannelDto, ChannelPatchDto, MAX_WIRE, MessageDto, PatchDto, Ready};
use libfuzzer_sys::fuzz_target;
use std::hint::black_box;

fuzz_target!(|data: &[u8]| {
    let Some((&selector, payload)) = data.split_first() else {
        return;
    };
    // The extra byte keeps the decoder's rejection boundary reachable.
    let payload = &payload[..payload.len().min(MAX_WIRE + 1)];
    if selector == u8::MAX {
        // Exercise the large boundary without committing a megabyte corpus entry or
        // requiring a fuzzer to grow a small seed to MAX_WIRE first.
        let mut boundary =
            br#"{"id":"3","channel_id":"2","author":{"id":"1","username":"Synthetic"}}"#.to_vec();
        boundary.resize(MAX_WIRE, b' ');
        black_box(
            discord_protocol::decode::<MessageDto>(&boundary)
                .expect("valid JSON at the wire limit")
                .into_model(),
        );
        boundary.push(b' ');
        assert!(discord_protocol::decode::<MessageDto>(&boundary).is_err());
        return;
    }
    // These are the same public decode/normalization boundaries used by the API
    // and Gateway. A decoded message need not pass the separate timeline policy.
    match selector % 8 {
        0 => {
            if let Ok(message) = discord_protocol::decode::<MessageDto>(payload) {
                black_box(message.into_model());
            }
        }
        1 => {
            if let Ok(patch) = discord_protocol::decode::<PatchDto>(payload) {
                black_box(patch.into_model());
            }
        }
        2 => {
            if let Ok(mut ready) = discord_protocol::decode::<Ready>(payload) {
                let _ = black_box(ready.navigation());
                black_box(ready.user.into_model());
                for user in ready.users {
                    black_box(user.into_model());
                }
            }
        }
        3 => {
            let _ = black_box(discord_protocol::permissions::ready(payload, model::Id(1)));
        }
        4 => {
            if let Ok(threads) =
                discord_protocol::decode::<discord_protocol::threads::ThreadListSync>(payload)
            {
                let _ = black_box(threads.into_model());
            }
        }
        5 => {
            // This entry point already normalizes status and custom activities.
            let _ = black_box(discord_protocol::presence::decode(payload));
        }
        6 => {
            if let Ok(channel) = discord_protocol::decode::<ChannelDto>(payload) {
                black_box(channel.into_model());
            }
        }
        7 => {
            if let Ok(patch) = discord_protocol::decode::<ChannelPatchDto>(payload) {
                black_box(patch.into_model());
            }
        }
        _ => unreachable!(),
    }
});
