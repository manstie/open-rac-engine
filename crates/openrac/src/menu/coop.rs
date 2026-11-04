//! The co-op page (PLAN.md P1.18): hosting a game, or joining one by the join code the
//! host shares (`crate::coop`).
//!
//! Joining reads the host's code, or its address, from the clipboard, since players pass
//! codes to each other by chat: there is no field to type into, whose letters the menu's
//! own keys would fight over.

use bevy::prelude::*;
use std::net::SocketAddr;

use super::{MenuCommand, PageRoot, button, label_text, menu_root, subtitle, title};
use crate::coop::parse_address;
use rac_net::code::JoinCode;

/// The line under the buttons: why the last join didn't start.
#[derive(Resource, Debug, Default)]
pub(super) struct CoopNotice(pub(super) String);

#[derive(Component, Debug)]
pub(super) struct CoopNoticeText;

pub(super) fn spawn(commands: &mut Commands, notice: &mut CoopNotice) {
    notice.0.clear();
    commands.spawn((
        PageRoot,
        menu_root(Color::srgb(0.05, 0.06, 0.08)),
        children![
            title("Co-op"),
            subtitle("The host shows a join code. Player 2 copies it, then chooses Join."),
            button(0, "Host a game", MenuCommand::Host),
            button(1, "Join with the copied code", MenuCommand::Join),
            label_text("", 18.0, CoopNoticeText),
            button(2, "Back", MenuCommand::Back),
        ],
    ));
}

pub(super) fn show_notice(
    notice: Res<CoopNotice>,
    mut texts: Query<&mut Text, With<CoopNoticeText>>,
) {
    for mut text in &mut texts {
        if text.0 != notice.0 {
            text.0.clone_from(&notice.0);
        }
    }
}

/// The host's addresses from what the clipboard holds: a join code, or an address.
pub(super) fn join_from(clipboard: Option<String>) -> Result<Vec<SocketAddr>, String> {
    let text = clipboard.ok_or("The clipboard holds no text: copy the host's code first.")?;
    let text = text.trim();
    if text.is_empty() {
        return Err("The clipboard is empty: copy the host's code first.".to_owned());
    }
    // Looking up anything else as a host name could hold the menu up.
    if text.len() > 253 || text.contains(char::is_whitespace) {
        return Err("The clipboard doesn't hold a join code or an address.".to_owned());
    }
    match JoinCode::decode(text) {
        Ok(code) => Ok(code.0),
        Err(error) => parse_address(text)
            .map(|address| vec![address])
            .map_err(|_| format!("That isn't a join code or an address ({error}).")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copied_code_or_address_is_joined_and_anything_else_is_said_why_not() {
        let hosts = vec![
            SocketAddr::from(([203, 0, 113, 10], 26450)),
            SocketAddr::from(([192, 168, 1, 20], 26450)),
        ];
        let code = JoinCode(hosts.clone()).encode();
        assert_eq!(join_from(Some(format!("  {code}\n"))), Ok(hosts));
        assert_eq!(
            join_from(Some("127.0.0.1:7000".into())),
            Ok(vec![SocketAddr::from(([127, 0, 0, 1], 7000))])
        );
        assert!(join_from(None).is_err());
        assert!(join_from(Some(" \n".into())).is_err());
        assert!(join_from(Some("see you at eight".into())).is_err());
    }
}
