use super::common::cli::{BrWorkspace, run_br};
use super::{create_issue, init_workspace, normalize_output};
use insta::assert_snapshot;

#[test]
fn snapshot_help_output() {
    let workspace = BrWorkspace::new();
    let output = run_br(&workspace, ["--help"], "help");
    assert!(output.status.success(), "help failed: {}", output.stderr);
    assert_snapshot!("help_output", normalize_output(&output.stdout));
}

#[test]
fn snapshot_create_help() {
    let workspace = BrWorkspace::new();
    let output = run_br(&workspace, ["create", "--help"], "create_help");
    assert!(
        output.status.success(),
        "create help failed: {}",
        output.stderr
    );
    assert_snapshot!("create_help", normalize_output(&output.stdout));
}

#[test]
fn snapshot_list_empty() {
    let workspace = init_workspace();
    let output = run_br(&workspace, ["list"], "list_empty");
    assert!(output.status.success(), "list failed: {}", output.stderr);
    assert_snapshot!("list_empty", normalize_output(&output.stdout));
}

#[test]
fn snapshot_list_with_issues() {
    let workspace = init_workspace();
    create_issue(&workspace, "Bug: Fix login", "create_bug");
    create_issue(&workspace, "Feature: Add dark mode", "create_feature");
    create_issue(&workspace, "Task: Update docs", "create_task");

    let output = run_br(&workspace, ["list"], "list_with_issues");
    assert!(output.status.success(), "list failed: {}", output.stderr);
    assert_snapshot!("list_with_issues", normalize_output(&output.stdout));
}

#[test]
fn snapshot_show_output() {
    let workspace = init_workspace();
    let id = create_issue(&workspace, "Test issue with description", "create_show");

    let output = run_br(&workspace, ["show", &id], "show_text");
    assert!(output.status.success(), "show failed: {}", output.stderr);
    assert_snapshot!("show_output", normalize_output(&output.stdout));
}

#[test]
fn snapshot_ready_output() {
    let workspace = init_workspace();
    // Create issues with different priorities using update
    let id1 = create_issue(&workspace, "Critical bug", "create_p0");
    let id2 = create_issue(&workspace, "High priority feature", "create_p1");
    let id3 = create_issue(&workspace, "Medium task", "create_p2");

    // Update priorities
    let _ = run_br(&workspace, ["update", &id1, "--priority", "0"], "update_p0");
    let _ = run_br(&workspace, ["update", &id2, "--priority", "1"], "update_p1");
    let _ = run_br(&workspace, ["update", &id3, "--priority", "2"], "update_p2");

    let output = run_br(&workspace, ["ready"], "ready_text");
    assert!(output.status.success(), "ready failed: {}", output.stderr);
    assert_snapshot!("ready_output", normalize_output(&output.stdout));
}

#[test]
fn snapshot_blocked_output() {
    let workspace = init_workspace();

    // Create dependency chain
    let blocker = create_issue(&workspace, "Database schema", "create_blocker");
    let blocked1 = create_issue(&workspace, "User model", "create_blocked1");
    let blocked2 = create_issue(&workspace, "Auth module", "create_blocked2");

    let _ = run_br(&workspace, ["dep", "add", &blocked1, &blocker], "dep_add1");
    let _ = run_br(&workspace, ["dep", "add", &blocked2, &blocked1], "dep_add2");

    let output = run_br(&workspace, ["blocked"], "blocked_text");
    assert!(output.status.success(), "blocked failed: {}", output.stderr);
    assert_snapshot!("blocked_output", normalize_output(&output.stdout));
}

#[test]
fn snapshot_stats_output() {
    let workspace = init_workspace();

    // Create mixed state issues
    let id1 = create_issue(&workspace, "Open issue 1", "create_open1");
    let id2 = create_issue(&workspace, "Open issue 2", "create_open2");
    let id3 = create_issue(&workspace, "Will close", "create_close");

    // Close one issue
    let _ = run_br(&workspace, ["close", &id3], "close_issue");

    // Add a dependency
    let _ = run_br(&workspace, ["dep", "add", &id2, &id1], "dep_add_stats");

    let output = run_br(&workspace, ["stats"], "stats_text");
    assert!(output.status.success(), "stats failed: {}", output.stderr);
    assert_snapshot!("stats_output", normalize_output(&output.stdout));
}

#[test]
fn snapshot_doctor_output() {
    let workspace = BrWorkspace::new();
    let init = run_br(&workspace, ["init"], "init");
    assert!(init.status.success());

    let output = run_br(&workspace, ["doctor"], "doctor");
    assert_snapshot!("doctor_output", normalize_output(&output.stdout));
}

#[test]
fn snapshot_version_output() {
    let workspace = BrWorkspace::new();
    let output = run_br(&workspace, ["version"], "version");
    assert_snapshot!("version_output", normalize_output(&output.stdout));
}
