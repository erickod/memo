mod common;

use common::run_git;
use std::cell::Cell;
use std::os::unix::process::ExitStatusExt;
use std::process::Command;

fn killed() -> Command {
    let mut c = Command::new("sh");
    c.args(["-c", "kill -9 $$"]);
    c
}

fn ok() -> Command {
    Command::new("true")
}

/// GIVEN a command killed by SIGKILL on the first attempt and fine on the second
/// WHEN  it runs through the harness helper
/// THEN  the helper repeats it once and returns the successful output
#[test]
fn test_given_command_killed_once_when_run_git_then_second_attempt_succeeds() {
    let calls = Cell::new(0);
    let out = run_git(|| {
        calls.set(calls.get() + 1);
        if calls.get() == 1 { killed() } else { ok() }
    });
    assert_eq!((out.status.success(), calls.get()), (true, 2));
}

/// GIVEN a command that is always killed by SIGKILL
/// WHEN  it runs through the harness helper
/// THEN  it stops after three attempts and returns the signal status
#[test]
fn test_given_command_always_killed_when_run_git_then_stops_after_three_attempts() {
    let calls = Cell::new(0);
    let out = run_git(|| {
        calls.set(calls.get() + 1);
        killed()
    });
    assert_eq!((out.status.signal(), calls.get()), (Some(9), 3));
}

/// GIVEN a command that exits with a non-zero code
/// WHEN  it runs through the harness helper
/// THEN  it is not repeated
#[test]
fn test_given_command_exiting_with_code_when_run_git_then_it_is_not_repeated() {
    let calls = Cell::new(0);
    let out = run_git(|| {
        calls.set(calls.get() + 1);
        Command::new("false")
    });
    assert_eq!((out.status.code(), calls.get()), (Some(1), 1));
}
