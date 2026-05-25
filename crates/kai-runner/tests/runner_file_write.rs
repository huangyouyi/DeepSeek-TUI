use deepseek_mobile_agent_core::{RemoteToolCall, RemoteToolName};
use kai_runner::KaiRunner;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

fn unique_workspace(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "kai-runner-file-write-{name}-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn write_call(arguments: serde_json::Value) -> RemoteToolCall {
    RemoteToolCall {
        call_id: "write-1".to_string(),
        name: RemoteToolName::FileWrite,
        arguments,
    }
}

fn write_text(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[test]
fn file_write_creates_file_inside_workspace() {
    let workspace = unique_workspace("create");
    let runner = KaiRunner::with_workspace(&workspace);

    let output = runner.execute(write_call(json!({
        "path": "notes/todo.txt",
        "content": "one\ntwo\n"
    })));

    assert!(output.success);
    assert_eq!(
        fs::read_to_string(workspace.join("notes/todo.txt")).unwrap(),
        "one\ntwo\n"
    );
    assert_eq!(
        output.result,
        json!({
            "tool": "remote.file.write",
            "status": "ok",
            "data": {
                "path": "notes/todo.txt",
                "mode": "overwrite",
                "bytes_written": 8,
                "backup_path": null
            }
        })
    );
}

#[test]
fn file_write_overwrites_existing_file_with_backup_by_default() {
    let workspace = unique_workspace("backup");
    write_text(&workspace, "notes.txt", "old content");
    let runner = KaiRunner::with_workspace(&workspace);

    let output = runner.execute(write_call(json!({
        "path": "notes.txt",
        "content": "new content"
    })));

    assert!(output.success);
    assert_eq!(
        fs::read_to_string(workspace.join("notes.txt")).unwrap(),
        "new content"
    );
    let backup_path = output.result["data"]["backup_path"].as_str().unwrap();
    assert!(backup_path.starts_with("notes.txt."));
    assert!(backup_path.ends_with(".bak"));
    assert_eq!(
        fs::read_to_string(workspace.join(backup_path)).unwrap(),
        "old content"
    );
}

#[test]
fn file_write_can_disable_backup() {
    let workspace = unique_workspace("no-backup");
    write_text(&workspace, "notes.txt", "old");
    let runner = KaiRunner::with_workspace(&workspace);

    let output = runner.execute(write_call(json!({
        "path": "notes.txt",
        "content": "new",
        "backup": false
    })));

    assert!(output.success);
    assert_eq!(
        fs::read_to_string(workspace.join("notes.txt")).unwrap(),
        "new"
    );
    assert_eq!(output.result["data"]["backup_path"], json!(null));
}

#[test]
fn file_write_appends_content() {
    let workspace = unique_workspace("append");
    write_text(&workspace, "notes.txt", "old\n");
    let runner = KaiRunner::with_workspace(&workspace);

    let output = runner.execute(write_call(json!({
        "path": "notes.txt",
        "content": "new\n",
        "mode": "append"
    })));

    assert!(output.success);
    assert_eq!(
        fs::read_to_string(workspace.join("notes.txt")).unwrap(),
        "old\nnew\n"
    );
    assert_eq!(output.result["data"]["mode"], json!("append"));
    assert_eq!(output.result["data"]["backup_path"], json!(null));
}

#[test]
fn file_write_rejects_paths_that_escape_workspace() {
    let workspace = unique_workspace("escape");
    let runner = KaiRunner::with_workspace(&workspace);

    let output = runner.execute(write_call(json!({
        "path": "../outside.txt",
        "content": "secret"
    })));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.file.write"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(output.result["error"]["code"], json!("path_escape"));
    assert!(!workspace.parent().unwrap().join("outside.txt").exists());
}

#[test]
fn default_runner_does_not_write_files() {
    let workspace = unique_workspace("default-runner");
    let target = workspace.join("notes.txt");
    let runner = KaiRunner;

    let output = runner.execute(write_call(json!({
        "path": target,
        "content": "nope"
    })));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.file.write"));
    assert_eq!(output.result["error"]["code"], json!("unsupported"));
    assert!(!target.exists());
}
