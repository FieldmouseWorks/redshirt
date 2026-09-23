use redshirt::{
    context::packet::{self, SourceRef, Spec},
    evidence::sha256,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

fn fixture() -> (TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    git(
        dir.path(),
        &["config", "user.email", "packet@example.invalid"],
    );
    git(dir.path(), &["config", "user.name", "Packet Test"]);
    fs::create_dir(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/a.txt"), b"first\r\nsecond\r\nthird").unwrap();
    fs::write(dir.path().join("src/b.txt"), b"candidate\n").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-qm", "source"]);
    let revision = git(dir.path(), &["rev-parse", "HEAD"]);
    (dir, revision)
}

fn source(id: &str, path: &str, start: usize, end: usize) -> SourceRef {
    SourceRef {
        id: id.into(),
        path: path.into(),
        start_line: start,
        end_line: end,
    }
}

fn spec(revision: &str) -> Spec {
    Spec {
        version: 1,
        source_revision: revision.into(),
        task: "Explain a synthetic source edge case.".into(),
        required: vec![source("must", "src/a.txt", 1, 2)],
        chunks: vec![
            source("tail", "src/a.txt", 3, 3),
            source("other", "src/b.txt", 1, 1),
        ],
    }
}

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_redshirt-packet"))
        .args(args)
        .env("GIT_DIR", "/does/not/exist")
        .env("GIT_WORK_TREE", "/does/not/exist")
        .env("GIT_INDEX_FILE", "/does/not/exist")
        .env("GIT_OBJECT_DIRECTORY", "/does/not/exist")
        .output()
        .unwrap()
}

fn path_text(path: &Path) -> String {
    path.to_str().unwrap().into()
}

#[test]
fn cli_build_verify_and_context_projection_preserve_every_original_byte() {
    let (repo, revision) = fixture();
    let spec_path = repo.path().join("spec.json");
    let packet_path = repo.path().join("packet.json");
    fs::write(&spec_path, serde_json::to_vec(&spec(&revision)).unwrap()).unwrap();
    let repo_arg = path_text(repo.path());
    let spec_arg = path_text(&spec_path);
    let packet_arg = path_text(&packet_path);
    let built = cli(&[
        "build",
        "--repo",
        &repo_arg,
        "--spec",
        &spec_arg,
        "--output",
        &packet_arg,
    ]);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let build_summary: Value = serde_json::from_slice(&built.stdout).unwrap();
    assert_eq!(build_summary["freshness"], "not_checked");
    assert_eq!(build_summary["required_count"], 1);
    assert_eq!(build_summary["chunk_count"], 2);
    assert_eq!(build_summary["source_count"], 2);
    assert!(!String::from_utf8_lossy(&built.stdout).contains("first"));

    let packet = packet::read_packet(&packet_path).unwrap();
    assert_eq!(packet.required[0].text.as_bytes(), b"first\r\nsecond\r\n");
    assert_eq!(packet.chunks[0].text.as_bytes(), b"third");
    assert_eq!(packet.chunks[1].text.as_bytes(), b"candidate\n");
    assert_eq!(
        packet.required[0].file_sha256,
        sha256(b"first\r\nsecond\r\nthird")
    );
    let (required, candidates) = packet.context_chunks();
    assert_eq!(required.len(), 1);
    assert_eq!(candidates.len(), 2);
    assert_eq!(required[0].id, "must");
    assert_eq!(required[0].text, packet.required[0].text);
    assert_eq!(candidates[0].text, "third");
    assert_eq!(candidates[1].text, "candidate\n");
    assert_eq!(
        required[0].source,
        format!("git:{revision}:src/a.txt#L1-L2")
    );

    let verified = cli(&["verify", "--repo", &repo_arg, "--packet", &packet_arg]);
    assert!(
        verified.status.success(),
        "{}",
        String::from_utf8_lossy(&verified.stderr)
    );
    let verify_summary: Value = serde_json::from_slice(&verified.stdout).unwrap();
    assert_eq!(verify_summary["freshness"], "current");
    assert_eq!(
        verify_summary["packet_sha256"],
        build_summary["packet_sha256"]
    );
    let second = cli(&[
        "build",
        "--repo",
        &repo_arg,
        "--spec",
        &spec_arg,
        "--output",
        &packet_arg,
    ]);
    assert!(!second.status.success());
    assert!(String::from_utf8_lossy(&second.stderr).contains("packet_output_exists"));
}

#[test]
fn history_build_is_readable_but_stale_head_fails_verification() {
    let (repo, revision) = fixture();
    fs::write(repo.path().join("unrelated.txt"), b"later\n").unwrap();
    git(repo.path(), &["add", "unrelated.txt"]);
    git(repo.path(), &["commit", "-qm", "later"]);
    let built = packet::build(repo.path(), &spec(&revision)).unwrap();
    assert_eq!(built.required[0].text, "first\r\nsecond\r\n");
    assert_eq!(built.build_summary().unwrap().freshness, "not_checked");
    assert_eq!(
        packet::verify(repo.path(), &built).unwrap_err().0,
        "packet_stale_head"
    );
}

#[test]
fn full_file_change_outside_excerpt_and_tamper_both_fail() {
    let (repo, revision) = fixture();
    let mut spec = spec(&revision);
    spec.chunks.clear();
    let packet = packet::build(repo.path(), &spec).unwrap();
    fs::write(repo.path().join("src/a.txt"), b"first\r\nsecond\r\nCHANGED").unwrap();
    assert_eq!(
        packet::verify(repo.path(), &packet).unwrap_err().0,
        "packet_stale_file"
    );
    fs::write(repo.path().join("src/a.txt"), b"first\r\nsecond\r\nthird").unwrap();
    let mut tampered = packet.clone();
    tampered.required[0].text = "first\r\nwrong\r\n".into();
    tampered.required[0].sha256 = sha256(tampered.required[0].text.as_bytes());
    tampered.validate().unwrap();
    assert_eq!(
        packet::verify(repo.path(), &tampered).unwrap_err().0,
        "packet_tampered"
    );
}

#[test]
fn malformed_input_identity_path_range_and_json_are_rejected() {
    let (repo, revision) = fixture();
    let mut bad = spec(&revision);
    bad.chunks[0].id = "must".into();
    assert_eq!(bad.validate().unwrap_err().0, "packet_duplicate_id");
    for path in [
        "/absolute",
        "../escape",
        "src/../a.txt",
        "src/./a.txt",
        "src//a.txt",
        ".git/config",
        "src\\a.txt",
        "src/a.txt/",
        "C:/other",
    ] {
        let mut bad = spec(&revision);
        bad.required[0].path = path.into();
        assert!(bad.validate().is_err(), "accepted {path}");
    }
    let mut bad = spec(&revision);
    bad.required[0].start_line = 0;
    assert!(bad.validate().is_err());
    bad.required[0].start_line = 3;
    bad.required[0].end_line = 2;
    assert!(bad.validate().is_err());
    bad.required[0].start_line = 4;
    bad.required[0].end_line = 4;
    assert_eq!(
        packet::build(repo.path(), &bad).unwrap_err().0,
        "packet_source_range"
    );

    let path = repo.path().join("bad-spec.json");
    fs::write(&path, format!("{{\"version\":1,\"version\":1,\"source_revision\":\"{revision}\",\"task\":\"x\",\"required\":[],\"chunks\":[]}}")).unwrap();
    assert!(packet::read_spec(&path).is_err());
    let mut value = serde_json::to_value(spec(&revision)).unwrap();
    value["unknown"] = json!(true);
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(packet::read_spec(&path).is_err());
    let packet = packet::build(repo.path(), &spec(&revision)).unwrap();
    let packet_path = repo.path().join("bad-packet.json");
    let raw = serde_json::to_string(&packet).unwrap();
    fs::write(
        &packet_path,
        raw.replacen("\"version\":1", "\"version\":1,\"version\":1", 1),
    )
    .unwrap();
    assert!(packet::read_packet(&packet_path).is_err());
}

#[test]
fn committed_symlink_gitlink_and_binary_sources_are_rejected() {
    let (repo, _) = fixture();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("a.txt", repo.path().join("src/link.txt")).unwrap();
    }
    fs::write(repo.path().join("src/nul.txt"), b"a\0b").unwrap();
    fs::write(repo.path().join("src/utf8.txt"), [0xff, 0xfe]).unwrap();
    fs::write(repo.path().join("src/empty.txt"), b"").unwrap();
    git(repo.path(), &["add", "."]);
    let earlier = git(repo.path(), &["rev-parse", "HEAD"]);
    git(
        repo.path(),
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{earlier},nested"),
        ],
    );
    git(repo.path(), &["commit", "-qm", "invalid sources"]);
    let revision = git(repo.path(), &["rev-parse", "HEAD"]);
    for (path, error) in [
        ("src/nul.txt", "packet_source_not_text"),
        ("src/utf8.txt", "packet_source_not_text"),
        ("src/empty.txt", "packet_source_not_text"),
        ("nested", "packet_source_not_regular"),
    ] {
        let mut input = spec(&revision);
        input.required = vec![source("one", path, 1, 1)];
        input.chunks.clear();
        assert_eq!(
            packet::build(repo.path(), &input).unwrap_err().0,
            error,
            "{path}"
        );
    }
    #[cfg(unix)]
    {
        let mut input = spec(&revision);
        input.required = vec![source("one", "src/link.txt", 1, 1)];
        input.chunks.clear();
        assert_eq!(
            packet::build(repo.path(), &input).unwrap_err().0,
            "packet_source_not_regular"
        );
    }
}

#[cfg(unix)]
#[test]
fn working_symlink_traversal_is_rejected_even_when_target_bytes_match() {
    let (repo, revision) = fixture();
    let packet = packet::build(repo.path(), &spec(&revision)).unwrap();
    fs::rename(repo.path().join("src"), repo.path().join("real-src")).unwrap();
    std::os::unix::fs::symlink("real-src", repo.path().join("src")).unwrap();
    assert_eq!(
        packet::verify(repo.path(), &packet).unwrap_err().0,
        "packet_stale_file"
    );
    fs::remove_file(repo.path().join("src")).unwrap();
    fs::rename(repo.path().join("real-src"), repo.path().join("src")).unwrap();
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("same.txt");
    fs::write(&outside_file, b"first\r\nsecond\r\nthird").unwrap();
    fs::remove_file(repo.path().join("src/a.txt")).unwrap();
    std::os::unix::fs::symlink(&outside_file, repo.path().join("src/a.txt")).unwrap();
    assert_eq!(
        packet::verify(repo.path(), &packet).unwrap_err().0,
        "packet_stale_file"
    );
}

#[test]
fn source_size_and_reference_count_are_hard_limits() {
    let (repo, revision) = fixture();
    let mut many = spec(&revision);
    many.required = (0..65)
        .map(|index| source(&format!("id{index}"), "src/a.txt", 1, 1))
        .collect();
    many.chunks.clear();
    assert!(many.validate().is_err());
    fs::write(
        repo.path().join("src/large.txt"),
        vec![b'x'; packet::MAX_SOURCE_BYTES + 1],
    )
    .unwrap();
    git(repo.path(), &["add", "src/large.txt"]);
    git(repo.path(), &["commit", "-qm", "large"]);
    let revision = git(repo.path(), &["rev-parse", "HEAD"]);
    let input = Spec {
        version: 1,
        source_revision: revision,
        task: "Bounded".into(),
        required: vec![source("large", "src/large.txt", 1, 1)],
        chunks: vec![],
    };
    assert_eq!(
        packet::build(repo.path(), &input).unwrap_err().0,
        "packet_git_output_limit"
    );
}

#[test]
fn cli_rejects_unknown_and_repeated_options() {
    let (repo, _) = fixture();
    let repo_arg: PathBuf = repo.path().into();
    let repo_arg = path_text(&repo_arg);
    for args in [
        vec!["build", "--repo", &repo_arg, "--repo", &repo_arg],
        vec!["verify", "--repo", &repo_arg, "--strange"],
    ] {
        let output = cli(&args);
        assert!(!output.status.success());
    }
}

#[test]
fn git_paths_with_pathspec_metacharacters_are_literal() {
    let (repo, _) = fixture();
    fs::write(repo.path().join("src/[odd]*.txt"), b"literal\n").unwrap();
    fs::write(repo.path().join("src/odd-other.txt"), b"different\n").unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-qm", "odd filenames"]);
    let revision = git(repo.path(), &["rev-parse", "HEAD"]);
    let input = Spec {
        version: 1,
        source_revision: revision,
        task: "Odd source path".into(),
        required: vec![source("odd", "src/[odd]*.txt", 1, 1)],
        chunks: vec![],
    };
    let packet = packet::build(repo.path(), &input).unwrap();
    assert_eq!(packet.required[0].text, "literal\n");
    packet::verify(repo.path(), &packet).unwrap();
}

#[test]
fn git_replace_objects_do_not_rewrite_pinned_provenance() {
    let (repo, original) = fixture();
    fs::write(repo.path().join("src/a.txt"), b"replacement\n").unwrap();
    git(repo.path(), &["add", "src/a.txt"]);
    git(repo.path(), &["commit", "-qm", "replacement"]);
    let newer = git(repo.path(), &["rev-parse", "HEAD"]);
    git(repo.path(), &["replace", &original, &newer]);
    let packet = packet::build(repo.path(), &spec(&original)).unwrap();
    assert_eq!(packet.required[0].text, "first\r\nsecond\r\n");
    assert_eq!(packet.chunks[0].text, "third");
    assert_eq!(
        packet::verify(repo.path(), &packet).unwrap_err().0,
        "packet_stale_head"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn verify_rejects_fifo_without_blocking() {
    let (repo, revision) = fixture();
    let packet = packet::build(repo.path(), &spec(&revision)).unwrap();
    let packet_path = repo.path().join("fifo-packet.json");
    packet::write_packet(&packet_path, &packet).unwrap();
    let source_path = repo.path().join("src/a.txt");
    fs::remove_file(&source_path).unwrap();
    let created = Command::new("mkfifo").arg(&source_path).output().unwrap();
    assert!(created.status.success());

    let mut child = Command::new(env!("CARGO_BIN_EXE_redshirt-packet"))
        .args([
            "verify",
            "--repo",
            repo.path().to_str().unwrap(),
            "--packet",
            packet_path.to_str().unwrap(),
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let started = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if started.elapsed() >= Duration::from_secs(1) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("verification blocked while opening a FIFO");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("packet_stale_file"));
}
