//! Integration tests covering CLI agent cwd propagation into per-tab metadata.
//!
//! When a CLI agent (Claude Code, Codex, etc.) reports a different working
//! directory via the `warp://cli-agent` OSC 777 channel, the vertical tab
//! badge and working-directory label should reflect the agent's cwd rather
//! than the shell's frozen prompt-cycle cwd. See upstream issue
//! `warpdotdev/warp#9125`.

use std::{path::PathBuf, time::Duration};

use warp::integration_testing::cli_agent::{end_session, inject_session};
use warp::integration_testing::step::new_step_with_default_assertions;
use warp::integration_testing::terminal::{
    execute_command_for_single_terminal_in_tab, util::ExpectedExitStatus,
    wait_until_bootstrapped_single_pane_for_tab,
};
use warp::integration_testing::view_getters::terminal_view;
use warp::terminal::CLIAgent;
use warpui::integration::{AssertionCallback, TestStep};
use warpui::{async_assert, async_assert_eq};

use crate::util::skip_if_powershell_core_2303;
use crate::Builder;

use super::new_builder;

const SHELL_BRANCH: &str = "main";
const WORKTREE_BRANCH: &str = "cli-agent-feature";
const WORKTREE_DIR: &str = "cli-agent-wt";

/// Asserts `TerminalView::current_git_branch` for a single-tab, single-pane test.
fn assert_current_git_branch(
    pane_index: usize,
    expected_branch: &'static str,
) -> AssertionCallback {
    Box::new(move |app, window_id| {
        let terminal_view = terminal_view(app, window_id, 0, pane_index);
        terminal_view.read(app, |terminal_view, ctx| {
            async_assert_eq!(
                terminal_view.current_git_branch(ctx),
                Some(expected_branch.to_string())
            )
        })
    })
}

/// Asserts `TerminalView::display_working_directory` ends with `expected_suffix`.
/// Suffix comparison avoids hardcoding the tmp test-root path.
fn assert_display_working_directory_ends_with(
    pane_index: usize,
    expected_suffix: &'static str,
) -> AssertionCallback {
    Box::new(move |app, window_id| {
        let terminal_view = terminal_view(app, window_id, 0, pane_index);
        terminal_view.read(app, |terminal_view, ctx| {
            let actual = terminal_view.display_working_directory(ctx);
            let matches = actual
                .as_deref()
                .is_some_and(|s| s.ends_with(expected_suffix));
            async_assert!(
                matches,
                "display_working_directory = {actual:?}, expected suffix = {expected_suffix}"
            )
        })
    })
}

fn inject_agent_session_step() -> TestStep {
    new_step_with_default_assertions("Inject CLI agent session at worktree cwd").with_action(
        |app, window_id, _| {
            let view_id = terminal_view(app, window_id, 0, 0).id();
            let home = std::env::var("HOME").expect("HOME should be set for integration tests");
            let cwd = PathBuf::from(home).join(WORKTREE_DIR);
            inject_session(app, view_id, CLIAgent::Claude, cwd);
        },
    )
}

fn end_agent_session_step() -> TestStep {
    new_step_with_default_assertions("End CLI agent session").with_action(|app, window_id, _| {
        let view_id = terminal_view(app, window_id, 0, 0).id();
        end_session(app, view_id);
    })
}

/// When a CLI agent reports a cwd that lives inside a different git worktree,
/// the tab's branch badge and working directory should follow the agent rather
/// than the shell. Ending the agent session reverts both to the shell's view.
pub fn test_cli_agent_cwd_updates_tab_metadata() -> Builder {
    new_builder()
        .set_should_run_test(skip_if_powershell_core_2303)
        .use_tmp_filesystem_for_test_root_directory()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(execute_command_for_single_terminal_in_tab(
            0,
            format!(
                "git init -b {SHELL_BRANCH}; \
                 git config user.email \"test@test.com\"; \
                 git config user.name \"Git TestUser\"; \
                 touch shell-file; \
                 git add . && git commit -m init"
            ),
            ExpectedExitStatus::Success,
            (),
        ))
        .with_step(execute_command_for_single_terminal_in_tab(
            0,
            format!("git worktree add ./{WORKTREE_DIR} -b {WORKTREE_BRANCH}"),
            ExpectedExitStatus::Success,
            (),
        ))
        .with_step(
            new_step_with_default_assertions("Shell branch metadata should be populated")
                .set_timeout(Duration::from_secs(15))
                .add_assertion(assert_current_git_branch(0, SHELL_BRANCH)),
        )
        .with_step(inject_agent_session_step())
        .with_step(
            new_step_with_default_assertions("Working directory should follow agent cwd")
                .set_timeout(Duration::from_secs(15))
                .add_assertion(assert_display_working_directory_ends_with(0, WORKTREE_DIR)),
        )
        .with_step(
            new_step_with_default_assertions("Git branch should follow agent cwd")
                .set_timeout(Duration::from_secs(15))
                .add_assertion(assert_current_git_branch(0, WORKTREE_BRANCH)),
        )
        .with_step(end_agent_session_step())
        .with_step(
            new_step_with_default_assertions("Tab metadata should revert to shell when agent ends")
                .set_timeout(Duration::from_secs(15))
                .add_assertion(assert_current_git_branch(0, SHELL_BRANCH)),
        )
}
