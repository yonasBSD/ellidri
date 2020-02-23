//! Client data, connection state and capability logic.

use crate::message::{Buffer, Command, MessageBuffer, ReplyBuffer};
use crate::modes;
use crate::util::time;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Clone, Debug)]
pub struct MessageQueueItem(Arc<str>);

impl From<String> for MessageQueueItem {
    fn from(bytes: String) -> Self {
        Self(Arc::from(bytes))
    }
}

impl From<Buffer> for MessageQueueItem {
    fn from(response: Buffer) -> Self {
        Self(Arc::from(response.build()))
    }
}

impl From<ReplyBuffer> for MessageQueueItem {
    fn from(response: ReplyBuffer) -> Self {
        Self(Arc::from(response.build()))
    }
}

impl AsRef<str> for MessageQueueItem {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl AsRef<[u8]> for MessageQueueItem {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref().as_bytes()
    }
}

pub type MessageQueue = mpsc::UnboundedSender<MessageQueueItem>;

/// A state machine that represent the connection with a client. It keeps track of what message the
/// client can send.
///
/// For example, a client that has only sent a "NICK" message cannot send a "JOIN" message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    ConnectionEstablished,
    NickGiven,
    UserGiven,
    CapGiven,
    CapNickGiven,
    CapUserGiven,
    CapNegotiation,
    Registered,
    Quit,
}

impl Default for ConnectionState {
    fn default() -> ConnectionState {
        ConnectionState::ConnectionEstablished
    }
}

impl ConnectionState {
    pub fn apply(self, command: Command, sub_command: &str) -> Result<ConnectionState, ()> {
        match self {
            ConnectionState::ConnectionEstablished => match command {
                Command::Cap if sub_command == "END" => Ok(self),
                Command::Cap if sub_command == "LS" => Ok(ConnectionState::CapGiven),
                Command::Cap if sub_command == "REQ" => Ok(ConnectionState::CapGiven),
                Command::Cap | Command::Pass => Ok(self),
                Command::Nick => Ok(ConnectionState::NickGiven),
                Command::User => Ok(ConnectionState::UserGiven),
                Command::Quit => Ok(ConnectionState::Quit),
                _ => Err(()),
            }
            ConnectionState::NickGiven => match command {
                Command::Cap if sub_command == "END" => Ok(self),
                Command::Cap if sub_command == "LS" => Ok(ConnectionState::CapNickGiven),
                Command::Cap if sub_command == "REQ" => Ok(ConnectionState::CapNickGiven),
                Command::Cap | Command::Nick | Command::Pass => Ok(self),
                Command::User => Ok(ConnectionState::Registered),
                Command::Quit => Ok(ConnectionState::Quit),
                _ => Err(()),
            }
            ConnectionState::UserGiven => match command {
                Command::Cap if sub_command == "END" => Ok(self),
                Command::Cap if sub_command == "LS" => Ok(ConnectionState::CapUserGiven),
                Command::Cap if sub_command == "REQ" => Ok(ConnectionState::CapUserGiven),
                Command::Cap | Command::Pass => Ok(self),
                Command::Nick => Ok(ConnectionState::Registered),
                Command::Quit => Ok(ConnectionState::Quit),
                _ => Err(()),
            }
            ConnectionState::CapGiven => match command {
                Command::Cap if sub_command == "END" => Ok(ConnectionState::ConnectionEstablished),
                Command::Cap | Command::Pass => Ok(self),
                Command::Nick => Ok(ConnectionState::CapNickGiven),
                Command::User => Ok(ConnectionState::CapUserGiven),
                Command::Quit => Ok(ConnectionState::Quit),
                _ => Err(()),
            }
            ConnectionState::CapNickGiven => match command {
                Command::Cap if sub_command == "END" => Ok(ConnectionState::NickGiven),
                Command::Cap | Command::Pass | Command::Nick => Ok(self),
                Command::User => Ok(ConnectionState::CapNegotiation),
                Command::Quit => Ok(ConnectionState::Quit),
                _ => Err(()),
            }
            ConnectionState::CapUserGiven => match command {
                Command::Cap if sub_command == "END" => Ok(ConnectionState::UserGiven),
                Command::Cap | Command::Pass => Ok(self),
                Command::Nick => Ok(ConnectionState::CapNegotiation),
                Command::Quit => Ok(ConnectionState::Quit),
                _ => Err(()),
            }
            ConnectionState::CapNegotiation => match command {
                Command::Cap if sub_command == "END" => Ok(ConnectionState::Registered),
                Command::Cap | Command::Pass | Command::Nick => Ok(self),
                Command::Quit => Ok(ConnectionState::Quit),
                _ => Err(()),
            }
            ConnectionState::Registered => match command {
                Command::Pass | Command::User => Err(()),
                Command::Quit => Ok(ConnectionState::Quit),
                _ => Ok(self),
            }
            ConnectionState::Quit => Err(()),
        }
    }

    pub fn is_registered(self) -> bool {
        self == ConnectionState::Registered
    }
}

// TODO factorize this with a macro?
pub mod cap {
    use std::collections::HashSet;

    pub const CAP_NOTIFY: &str   = "cap-notify";
    pub const ECHO_MESSAGE: &str = "echo-message";
    pub const MESSAGE_TAGS: &str = "message-tags";

    // TODO replace with const fn
    lazy_static::lazy_static! {
        pub static ref ALL: HashSet<&'static str> =
            [ CAP_NOTIFY
            , ECHO_MESSAGE
            , MESSAGE_TAGS
            ].iter().cloned().collect();
    }

    pub const LS: &str = "cap-notify echo-message message-tags";

    pub fn are_supported(capabilities: &str) -> bool {
        super::cap_query(capabilities).all(|(cap,  _)| ALL.contains(cap))
    }
}


#[derive(Default)]
pub struct Capabilities {
    pub v302: bool,
    pub cap_notify: bool,
    pub echo_message: bool,
    pub message_tags: bool,
}

fn cap_query(buf: &str) -> impl Iterator<Item=(&str, bool)> {
    buf.split_whitespace().map(|word| {
        if word.starts_with('-') {
            (&word[1..], false)
        } else {
            (word, true)
        }
    })
}

const FULL_NAME_LENGTH: usize = 63;

/// Client data.
pub struct Client {
    /// The queue of messages to be sent to the client.
    ///
    /// This is the write end of a mpsc channel of messages (similar to go channels). It is
    /// currently unbounded, meaning sending messages to this channel do not block.
    queue: MessageQueue,

    pub capabilities: Capabilities,
    state: ConnectionState,

    nick: String,
    user: String,
    real: String,
    host: String,

    /// The nick!user@host
    full_name: String,

    /// The time when the user has signed in
    signon_time: u64,

    /// The time of the last action
    last_action_time: u64,

    /// Whether the client has issued a PASS command with the right password.
    pub has_given_password: bool,

    // Modes: https://tools.ietf.org/html/rfc2812.html#section-3.1.5
    pub away: bool,
    pub invisible: bool,
    pub registered: bool,
    pub operator: bool,
}

impl Client {
    /// Initialize the data for a new client, given its message queue.
    ///
    /// The nickname is set to "*", as it seems it's what freenode server does.  The username and
    /// the realname are set to empty strings.
    pub fn new(queue: MessageQueue, host: String) -> Self {
        let now = time();
        let mut full_name = String::with_capacity(FULL_NAME_LENGTH);
        full_name.push('*');
        full_name.push_str(&host);
        Self {
            queue,
            nick: String::from("*"),
            host,
            full_name,
            capabilities: Capabilities::default(),
            state: ConnectionState::default(),
            user: String::new(),
            real: String::new(),
            signon_time: now,
            last_action_time: now,
            has_given_password: false,
            away: false,
            invisible: false,
            registered: false,
            operator: false,
        }
    }

    pub fn state(&self) -> ConnectionState {
        self.state
    }

    // TODO factorize this with a macro?
    pub fn update_capabilities(&mut self, capabilities: &str) {
        for (capability, enable) in cap_query(capabilities) {
            match capability {
                cap::CAP_NOTIFY => self.capabilities.cap_notify = enable,
                cap::ECHO_MESSAGE => self.capabilities.echo_message = enable,
                cap::MESSAGE_TAGS => self.capabilities.message_tags = enable,
                _ => {}
            }
        }
    }

    pub fn set_cap_version(&mut self, version: &str) {
        if version == "302" {
            self.capabilities.v302 = true;
            self.capabilities.cap_notify = true;
        }
    }

    // TODO factorize this with a macro?
    pub fn write_enabled_capabilities(&self, response: &mut ReplyBuffer) {
        let mut msg = response.reply(Command::Cap).param("LIST");
        let trailing = msg.raw_trailing_param();
        if self.capabilities.cap_notify {
            trailing.push_str(cap::CAP_NOTIFY);
            trailing.push(' ');
        }
        if self.capabilities.echo_message {
            trailing.push_str(cap::ECHO_MESSAGE);
            trailing.push(' ');
        }
        if self.capabilities.message_tags {
            trailing.push_str(cap::MESSAGE_TAGS);
            trailing.push(' ');
        }
        trailing.pop();
    }

    pub fn write_capabilities(&self, response: &mut ReplyBuffer) {
        response.reply(Command::Cap).param("LS").trailing_param(cap::LS);
    }

    /// Change the connection state of the client given the command it just sent.
    ///
    /// # Panics
    ///
    /// This function panics if the command cannot be issued in the client current state.
    /// `Client::can_issue_command` should be called before.
    pub fn apply_command(&mut self, command: Command, sub_command: &str) -> ConnectionState {
        self.state = self.state.apply(command, sub_command).unwrap();
        self.state
    }

    /// Whether or not the client can issue the given command.
    ///
    /// This function does not change the connection state.
    pub fn can_issue_command(&self, command: Command, sub_command: &str) -> bool {
        self.state.apply(command, sub_command).is_ok()
    }

    pub fn is_registered(&self) -> bool {
        self.state == ConnectionState::Registered
    }

    /// Add a message to the client message queue.
    ///
    /// Use this function to send messages to the client.
    pub fn send<M>(&self, msg: M)
        where M: Into<MessageQueueItem>
    {
        let _ = self.queue.send(msg.into());
    }

    pub fn full_name(&self) -> &str {
        &self.full_name
    }

    fn update_full_name(&mut self) {
        self.full_name.clear();
        self.full_name.push_str(&self.nick);
        self.full_name.push('!');
        self.full_name.push_str(&self.user);
        self.full_name.push('@');
        self.full_name.push_str(&self.host);
    }

    /// The nickname of the client
    pub fn nick(&self) -> &str {
        &self.nick
    }

    /// Change the nickname of the client.
    ///
    /// This function does not change the connection state.
    pub fn set_nick(&mut self, nick: &str) {
        self.nick.clear();
        self.nick.push_str(nick);
        self.update_full_name();
    }

    /// The username of the client
    pub fn user(&self) -> &str {
        &self.user
    }

    /// The realname of the client
    pub fn real(&self) -> &str {
        &self.real
    }

    /// The host of the client
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Change the username and the realname of the client.
    ///
    /// This function does not change the connection state.
    pub fn set_user_real(&mut self, user: &str, real: &str) {
        self.user.push_str(user);
        self.real.push_str(real);
        self.update_full_name();
    }

    pub fn signon_time(&self) -> u64 {
        self.signon_time
    }

    pub fn idle_time(&self) -> u64 {
        time() - self.last_action_time
    }

    pub fn update_idle_time(&mut self) {
        self.last_action_time = time();
    }

    pub fn write_modes(&self, mut out: MessageBuffer<'_>) {
        let modes = out.raw_param();
        modes.push('+');
        if self.away { modes.push('a'); }
        if self.invisible { modes.push('i'); }
        if self.operator { modes.push('o'); }
    }

    pub fn apply_mode_change(&mut self, change: modes::UserModeChange) -> bool {
        use modes::UserModeChange::*;
        let applied;
        match change {
            Invisible(value) => {
                applied = self.invisible != value;
                self.invisible = value;
            },
        }
        applied
    }
}
