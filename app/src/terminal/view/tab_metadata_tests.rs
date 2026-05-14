//! Unit tests for `TerminalView::display_working_directory` and the agent-cwd
//! override added for upstream issue #9125.

use warpui::{App, EntityId, SingletonEntity};

use crate::terminal::cli_agent_sessions::{
    CLIAgentInputState, CLIAgentSession, CLIAgentSessionContext, CLIAgentSessionStatus,
    CLIAgentSessionsModel,
};
use crate::terminal::CLIAgent;
use crate::test_util::add_window_with_terminal;
use crate::test_util::terminal::initialize_app_for_terminal_view;

fn inject_agent_cwd(app: &mut App, view_id: EntityId, cwd: &str) {
    CLIAgentSessionsModel::handle(app).update(app, |sessions, ctx| {
        let session = CLIAgentSession {
            agent: CLIAgent::Claude,
            status: CLIAgentSessionStatus::InProgress,
            session_context: CLIAgentSessionContext {
                cwd: Some(cwd.to_string()),
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

fn end_agent_session(app: &mut App, view_id: EntityId) {
    CLIAgentSessionsModel::handle(app).update(app, |sessions, ctx| {
        sessions.remove_session(view_id, ctx);
    });
}

#[test]
fn display_working_directory_prefers_agent_cwd() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);
        let view_id = terminal.id();

        inject_agent_cwd(&mut app, view_id, "/tmp/cli-agent-wt");

        let observed = terminal.read(&app, |view, ctx| view.display_working_directory(ctx));
        assert_eq!(observed.as_deref(), Some("/tmp/cli-agent-wt"));
    });
}

#[test]
fn display_working_directory_treats_blank_agent_cwd_as_absent() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);
        let view_id = terminal.id();

        inject_agent_cwd(&mut app, view_id, "   ");

        let observed = terminal.read(&app, |view, ctx| view.display_working_directory(ctx));
        assert_eq!(observed, None);
    });
}

#[test]
fn display_working_directory_reverts_when_agent_session_ends() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);
        let view_id = terminal.id();

        inject_agent_cwd(&mut app, view_id, "/tmp/cli-agent-wt");
        let during = terminal.read(&app, |view, ctx| view.display_working_directory(ctx));
        assert_eq!(during.as_deref(), Some("/tmp/cli-agent-wt"));

        end_agent_session(&mut app, view_id);
        let after = terminal.read(&app, |view, ctx| view.display_working_directory(ctx));
        assert_eq!(after, None);
    });
}

#[test]
fn current_git_branch_returns_none_without_agent_or_shell_metadata() {
    App::test((), |mut app| async move {
        initialize_app_for_terminal_view(&mut app);
        let terminal = add_window_with_terminal(&mut app, None);

        let observed = terminal.read(&app, |view, ctx| view.current_git_branch(ctx));
        assert_eq!(observed, None);
    });
}
