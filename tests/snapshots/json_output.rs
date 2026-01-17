use super::common::cli::run_br;
use super::{create_issue, init_workspace, normalize_json};
use insta::assert_json_snapshot;
use serde_json::Value;

#[test]
fn snapshot_list_json() {
    let workspace = init_workspace();
    create_issue(&workspace, "Issue one", "create_one");
    create_issue(&workspace, "Issue two", "create_two");

    let output = run_br(&workspace, ["list", "--json"], "list_json");
    assert!(
        output.status.success(),
        "list json failed: {}",
        output.stderr
    );

    let json: Value = serde_json::from_str(&output.stdout).expect("parse json");
    assert_json_snapshot!("list_json_output", normalize_json(&json));
}

#[test]
fn snapshot_show_json() {
    let workspace = init_workspace();
    let id = create_issue(&workspace, "Detailed issue", "create_detail");

    let output = run_br(&workspace, ["show", &id, "--json"], "show_json");
    assert!(
        output.status.success(),
        "show json failed: {}",
        output.stderr
    );

    let json: Value = serde_json::from_str(&output.stdout).expect("parse json");
    assert_json_snapshot!("show_json_output", normalize_json(&json));
}

#[test]
fn snapshot_ready_json() {
    let workspace = init_workspace();
    create_issue(&workspace, "Ready issue", "create_ready");

    let output = run_br(&workspace, ["ready", "--json"], "ready_json");
    assert!(
        output.status.success(),
        "ready json failed: {}",
        output.stderr
    );

    let json: Value = serde_json::from_str(&output.stdout).expect("parse json");
    assert_json_snapshot!("ready_json_output", normalize_json(&json));
}

#[test]
#[allow(clippy::similar_names)]
fn snapshot_blocked_json() {
    let workspace = init_workspace();

    // Create a dependency chain
    let blocker = create_issue(&workspace, "Blocker issue", "create_blocker_json");
    let blocked = create_issue(&workspace, "Blocked issue", "create_blocked_json");

    let _ = run_br(
        &workspace,
        ["dep", "add", &blocked, &blocker],
        "dep_add_json",
    );

    let output = run_br(&workspace, ["blocked", "--json"], "blocked_json");
    assert!(
        output.status.success(),
        "blocked json failed: {}",
        output.stderr
    );

    let json: Value = serde_json::from_str(&output.stdout).expect("parse json");
    assert_json_snapshot!("blocked_json_output", normalize_json(&json));
}

#[test]
fn snapshot_list_with_filters_json() {
    let workspace = init_workspace();
    let id1 = create_issue(&workspace, "Bug: Fix login", "create_bug_json");
    let id2 = create_issue(&workspace, "Feature: Add theme", "create_feature_json");

    // Update types
    let _ = run_br(
        &workspace,
        ["update", &id1, "--type", "bug"],
        "update_bug_json",
    );
    let _ = run_br(
        &workspace,
        ["update", &id2, "--type", "feature"],
        "update_feature_json",
    );

    // List only bugs
    let output = run_br(
        &workspace,
        ["list", "--type", "bug", "--json"],
        "list_bugs_json",
    );
    assert!(
        output.status.success(),
        "list bugs json failed: {}",
        output.stderr
    );

    let json: Value = serde_json::from_str(&output.stdout).expect("parse json");
    assert_json_snapshot!("list_filtered_json_output", normalize_json(&json));
}

#[test]
fn snapshot_stats_json() {
    let workspace = init_workspace();
    create_issue(&workspace, "Stats Issue", "create_stats");

    let output = run_br(&workspace, ["stats", "--json"], "stats_json");
    assert!(output.status.success());
    // Parse the JSON string into Value before passing to normalize_json
    let json: serde_json::Value = serde_json::from_str(&output.stdout).expect("parse json");
    assert_json_snapshot!("stats_json_output", normalize_json(&json));
}

#[test]
fn snapshot_create_json() {
    let workspace = init_workspace();

    let output = run_br(
        &workspace,
        [
            "create",
            "New feature request",
            "--type",
            "feature",
            "--priority",
            "1",
            "--json",
        ],
        "create_json",
    );
    assert!(
        output.status.success(),
        "create json failed: {}",
        output.stderr
    );

    let json: Value = serde_json::from_str(&output.stdout).expect("parse json");
    assert_json_snapshot!("create_json_output", normalize_json(&json));
}

#[test]
fn snapshot_update_json() {
    let workspace = init_workspace();
    let id = create_issue(&workspace, "Issue to update", "create_update");

    let output = run_br(
        &workspace,
        ["update", &id, "--status", "in_progress", "--json"],
        "update_json",
    );
    assert!(
        output.status.success(),
        "update json failed: {}",
        output.stderr
    );

    let json: Value = serde_json::from_str(&output.stdout).expect("parse json");
    assert_json_snapshot!("update_json_output", normalize_json(&json));
}

#[test]
fn snapshot_close_json() {
    let workspace = init_workspace();
    let id = create_issue(&workspace, "Issue to close", "create_close_json");

    let output = run_br(
        &workspace,
        ["close", &id, "--reason", "Done", "--json"],
        "close_json",
    );
    assert!(
        output.status.success(),
        "close json failed: {}",
        output.stderr
    );

    let json: Value = serde_json::from_str(&output.stdout).expect("parse json");
    assert_json_snapshot!("close_json_output", normalize_json(&json));
}

#[test]
fn snapshot_dep_list_json() {
    let workspace = init_workspace();
    let id1 = create_issue(&workspace, "Parent issue", "create_parent");
    let id2 = create_issue(&workspace, "Child issue", "create_child");

    // Add dependency
    let add = run_br(&workspace, ["dep", "add", &id2, &id1], "dep_add");
    assert!(add.status.success(), "dep add failed: {}", add.stderr);

    let output = run_br(&workspace, ["dep", "list", &id2, "--json"], "dep_list_json");
    assert!(
        output.status.success(),
        "dep list json failed: {}",
        output.stderr
    );

    let json: Value = serde_json::from_str(&output.stdout).expect("parse json");
    assert_json_snapshot!("dep_list_json_output", normalize_json(&json));
}
