//! Test helpers for driving `CLIAgentSessionsModel` from integration tests
//! without going through the OSC 777 parser or constructing a real listener.

use std::path::PathBuf;

use warpui::{App, EntityId, SingletonEntity};

use crate::terminal::cli_agent_sessions::{
    CLIAgentInputState, CLIAgentSession, CLIAgentSessionContext, CLIAgentSessionStatus,
    CLIAgentSessionsModel,
};
use crate::terminal::CLIAgent;

/// Inject (or replace) a CLI agent session for the given terminal view, carrying
/// a `cwd` that mimics what the agent plugin would report via OSC 777. The
/// session has no listener — it exists purely so that consumers reading
/// `session_context.cwd` see the test-supplied value.
pub fn inject_session(app: &mut App, view_id: EntityId, agent: CLIAgent, cwd: PathBuf) {
    CLIAgentSessionsModel::handle(app).update(app, |sessions, ctx| {
        let session = CLIAgentSession {
            agent,
            status: CLIAgentSessionStatus::InProgress,
            session_context: CLIAgentSessionContext {
                cwd: Some(cwd.to_string_lossy().into_owned()),
                ..Default::default()
            },
            input_state: CLIAgentInputState::Closed,
            should_auto_toggle_input: false,
            listener: None,
            plugin_version: None,
            remote_host: None,
            draft_text: None,
            custom_command_prefix: None,
        };
        sessions.set_session(view_id, session, ctx);
    });
}

/// End the CLI agent session associated with `view_id`. Subscribers receive
/// `CLIAgentSessionsModelEvent::Ended` and should revert any agent-driven state.
pub fn end_session(app: &mut App, view_id: EntityId) {
    CLIAgentSessionsModel::handle(app).update(app, |sessions, ctx| {
        sessions.remove_session(view_id, ctx);
    });
}
