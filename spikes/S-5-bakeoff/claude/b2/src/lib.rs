//! S-5 bake-off B-2: typed parsing of Twitch EventSub WebSocket messages.
//!
//! Entry point: [`parse_message`] turns one WebSocket text frame into a [`Message`].
//! Every type here serializes back to JSON equal to what it was parsed from (one exception:
//! a nullable field Twitch omits entirely comes back as `null`; see README.md):
//!
//! - unknown `message_type` values become [`Message::Unknown`];
//! - a known `message_type` with a malformed envelope becomes [`Message::Malformed`];
//! - unknown subscription types or versions become [`Event::Raw`];
//! - a known event that fails typed parsing also becomes [`Event::Raw`], with the
//!   error kept in [`RawEvent::parse_error`], so a Twitch schema change never drops an event;
//! - fields Twitch adds later land in each struct's `extra` map.
//!
//! Spec: <https://dev.twitch.tv/docs/eventsub/handling-websocket-events/> and
//! <https://dev.twitch.tv/docs/eventsub/eventsub-reference/>.

#[macro_use]
mod string_enum;

pub mod events;
pub mod message;

pub use events::*;
pub use message::*;

/// Parses one EventSub WebSocket text frame.
///
/// Fails only when the frame is not a JSON object with `metadata.message_type`, i.e. not an
/// EventSub message at all.
pub fn parse_message(text: &str) -> Result<Message, serde_json::Error> {
    serde_json::from_str(text)
}
