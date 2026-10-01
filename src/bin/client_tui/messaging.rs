//! TG-1205: the remaining message operations — quote replies, silent and scheduled sends,
//! in-chat search, joining by invite link. State and contracts live here together with the
//! composer's slash commands; `dispatch_messaging` runs the requests, `update_messaging` handles
//! keys and answers, `render_messaging` draws the two list dialogs.

use chrono::{DateTime, Duration, Local, NaiveDateTime, NaiveTime, TimeZone, Utc};
use uuid::Uuid;

use crate::client_api::ApiResult;
use crate::client_api_messages::{FoundMessage, InviteJoin, ScheduledMessage};

#[derive(Clone, Debug)]
pub enum MessagingAction {
    LoadScheduled(Uuid),
    Schedule {
        room_id: Uuid,
        content: String,
        at: DateTime<Utc>,
        silent: bool,
    },
    SendScheduledNow {
        room_id: Uuid,
        id: Uuid,
    },
    CancelScheduled {
        room_id: Uuid,
        id: Uuid,
    },
    Search {
        room_id: Uuid,
        password: Option<String>,
        query: String,
    },
    JoinInvite(String),
}

#[derive(Debug)]
pub enum MessagingEvent {
    Scheduled {
        room_id: Uuid,
        result: ApiResult<Vec<ScheduledMessage>>,
    },
    /// A schedule / send-now / cancel finished; the text is the status line, then the list
    /// reloads.
    ScheduledChanged {
        room_id: Uuid,
        result: ApiResult<String>,
    },
    Found {
        query: String,
        result: ApiResult<Vec<FoundMessage>>,
    },
    Joined(ApiResult<InviteJoin>),
}

/// What Enter in the composer means once slash commands are taken into account.
#[derive(Clone, Debug, PartialEq)]
pub enum ComposeCommand {
    Send {
        content: String,
        silent: bool,
    },
    Schedule {
        content: String,
        at: DateTime<Utc>,
        silent: bool,
    },
    ListScheduled,
    Search(String),
    Join(String),
    /// A recognised command used wrongly; the text is the status line.
    Usage(&'static str),
}

pub const SCHEDULE_USAGE: &str =
    "Usage: /schedule <30m|2h|1d|HH:MM|YYYY-MM-DD HH:MM> [/silent] <message>";

/// Parses the composer text. Only the known commands are intercepted; any other text (including
/// an unknown `/word`) is an ordinary message, and `//text` sends `/text` literally.
pub fn parse_compose(input: &str, now: DateTime<Local>) -> ComposeCommand {
    let text = input.trim();
    if let Some(literal) = text.strip_prefix("//") {
        return ComposeCommand::Send {
            content: format!("/{literal}"),
            silent: false,
        };
    }
    let (command, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let rest = rest.trim();
    match command {
        "/silent" if rest.is_empty() => ComposeCommand::Usage("Usage: /silent <message>"),
        "/silent" => ComposeCommand::Send {
            content: rest.into(),
            silent: true,
        },
        "/scheduled" => ComposeCommand::ListScheduled,
        "/schedule" => parse_schedule(rest, now),
        "/search" if rest.is_empty() => ComposeCommand::Usage("Usage: /search <text>"),
        "/search" => ComposeCommand::Search(rest.into()),
        "/join" if rest.is_empty() => ComposeCommand::Usage("Usage: /join <invite link>"),
        "/join" => ComposeCommand::Join(rest.into()),
        _ => ComposeCommand::Send {
            content: text.into(),
            silent: false,
        },
    }
}

fn parse_schedule(rest: &str, now: DateTime<Local>) -> ComposeCommand {
    let words: Vec<&str> = rest.split_whitespace().collect();
    // `YYYY-MM-DD HH:MM` takes two words, everything else one.
    let (at, used) = match words.as_slice() {
        [date, time, ..] if date.contains('-') => (when(&format!("{date} {time}"), now), 2),
        [first, ..] => (when(first, now), 1),
        [] => (None, 0),
    };
    let Some(at) = at else {
        return ComposeCommand::Usage(SCHEDULE_USAGE);
    };
    let mut body = &words[used..];
    let silent = body.first() == Some(&"/silent");
    if silent {
        body = &body[1..];
    }
    if body.is_empty() {
        return ComposeCommand::Usage(SCHEDULE_USAGE);
    }
    if at <= now.with_timezone(&Utc) {
        return ComposeCommand::Usage("That time has already passed");
    }
    ComposeCommand::Schedule {
        content: body.join(" "),
        at,
        silent,
    }
}

/// `30m`, `2h`, `1d` from now; `HH:MM` today (tomorrow once passed); `YYYY-MM-DD HH:MM` local.
pub fn when(value: &str, now: DateTime<Local>) -> Option<DateTime<Utc>> {
    if let Some(number) = value.strip_suffix(['m', 'h', 'd']) {
        let amount: i64 = number.parse().ok().filter(|n| *n > 0)?;
        let delta = match value.chars().last()? {
            'm' => Duration::minutes(amount),
            'h' => Duration::hours(amount),
            _ => Duration::days(amount),
        };
        return Some((now + delta).with_timezone(&Utc));
    }
    if let Ok(time) = NaiveTime::parse_from_str(value, "%H:%M") {
        let today = now.date_naive().and_time(time);
        let mut at = Local.from_local_datetime(&today).earliest()?;
        if at <= now {
            at = Local
                .from_local_datetime(&(today + Duration::days(1)))
                .earliest()?;
        }
        return Some(at.with_timezone(&Utc));
    }
    let naive = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M").ok()?;
    Some(
        Local
            .from_local_datetime(&naive)
            .earliest()?
            .with_timezone(&Utc),
    )
}

/// Local `MM-DD HH:MM` for a server timestamp (the raw text when it does not parse).
pub fn local_time(value: &str) -> String {
    DateTime::parse_from_rfc3339(value).map_or_else(
        |_| value.to_string(),
        |time| time.with_timezone(&Local).format("%m-%d %H:%M").to_string(),
    )
}

#[cfg(test)]
#[path = "messaging_tests.rs"]
mod tests;
