use deepseek_mobile_agent_core::{RemoteToolCall, RemoteToolName};
use kai_runner::KaiRunner;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

fn unique_workspace(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "kai-runner-{name}-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn call(name: RemoteToolName, arguments: serde_json::Value) -> RemoteToolCall {
    RemoteToolCall {
        call_id: format!("call-{name}"),
        name,
        arguments,
    }
}

fn read_call(arguments: serde_json::Value) -> RemoteToolCall {
    call(RemoteToolName::FileRead, arguments)
}

fn write_text(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[test]
fn file_read_returns_utf8_text_from_workspace() {
    let workspace = unique_workspace("read-text");
    write_text(&workspace, "notes/todo.txt", "one\ntwo\nthree\n");
    let runner = KaiRunner::with_workspace(&workspace);

    let output = runner.execute(read_call(json!({ "path": "notes/todo.txt" })));

    assert!(output.success);
    assert_eq!(output.call_id, "call-remote.file.read");
    assert_eq!(
        output.result,
        json!({
            "tool": "remote.file.read",
            "status": "ok",
            "data": {
                "path": "notes/todo.txt",
                "content": "one\ntwo\nthree\n",
                "truncated": false,
                "total_lines": 3,
                "start_line": 1,
                "next_start_line": null
            }
        })
    );
}

#[test]
fn file_read_supports_start_line_and_max_lines() {
    let workspace = unique_workspace("read-lines");
    write_text(&workspace, "log.txt", "alpha\nbeta\ngamma\ndelta\n");
    let runner = KaiRunner::with_workspace(&workspace);

    let output = runner.execute(read_call(json!({
        "path": "log.txt",
        "start_line": 2,
        "max_lines": 2
    })));

    assert!(output.success);
    assert_eq!(
        output.result["data"],
        json!({
            "path": "log.txt",
            "content": "beta\ngamma\n",
            "truncated": true,
            "total_lines": 4,
            "start_line": 2,
            "next_start_line": 4
        })
    );
}

#[test]
fn file_read_rejects_paths_that_escape_workspace() {
    let workspace = unique_workspace("escape-root");
    let outside = workspace.parent().unwrap().join("kai-runner-secret.txt");
    fs::write(&outside, "secret").unwrap();
    let runner = KaiRunner::with_workspace(&workspace);

    let output = runner.execute(read_call(json!({ "path": "../kai-runner-secret.txt" })));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.file.read"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(output.result["error"]["code"], json!("path_escape"));
}

#[test]
fn file_read_rejects_binary_files() {
    let workspace = unique_workspace("binary");
    fs::write(workspace.join("data.bin"), [0xff, 0xfe, 0x00, 0x61]).unwrap();
    let runner = KaiRunner::with_workspace(&workspace);

    let output = runner.execute(read_call(json!({ "path": "data.bin" })));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.file.read"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(output.result["error"]["code"], json!("not_utf8"));
}

#[test]
fn file_write_remains_unsupported_for_default_runner() {
    let workspace = unique_workspace("write-default-runner");
    let target = workspace.join("notes.txt");
    let runner = KaiRunner;

    let output = runner.execute(call(
        RemoteToolName::FileWrite,
        json!({ "path": target, "content": "nope" }),
    ));

    assert!(!output.success);
    assert_eq!(output.result["tool"], json!("remote.file.write"));
    assert_eq!(output.result["status"], json!("error"));
    assert_eq!(output.result["error"]["code"], json!("unsupported"));
    assert!(!target.exists());
}
