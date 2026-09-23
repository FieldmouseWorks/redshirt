//! Caller-selected source evidence for one coding task.
//!
//! A packet binds excerpts to committed Git blobs. Verification checks current
//! working files sequentially; it is not an atomic filesystem snapshot or an
//! assertion that the caller supplied every relevant source.

use crate::{
    Result, Stop,
    context::Chunk,
    evidence::{encoded, load, sha256},
    require,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

pub const MAX_SPEC_BYTES: usize = 64 * 1024;
pub const MAX_PACKET_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAX_TOTAL_SOURCE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_REFS: usize = 64;
pub const MAX_TASK_BYTES: usize = 8 * 1024;
pub const MAX_PATH_BYTES: usize = 400;
const GIT_TIMEOUT: Duration = Duration::from_secs(5);
const GIT_STDERR_BYTES: usize = 8192;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    pub id: String,
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub version: u32,
    pub source_revision: String,
    pub task: String,
    pub required: Vec<SourceRef>,
    pub chunks: Vec<SourceRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceExcerpt {
    #[serde(flatten)]
    pub source: SourceRef,
    pub file_sha256: String,
    pub text: String,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Packet {
    pub version: u32,
    pub source_revision: String,
    pub task: String,
    pub required: Vec<SourceExcerpt>,
    pub chunks: Vec<SourceExcerpt>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub packet_sha256: String,
    pub freshness: String,
    pub required_count: usize,
    pub chunk_count: usize,
    pub source_count: usize,
}

fn digest(value: &impl Serialize) -> Result<String> {
    Ok(sha256(&encoded(value)?))
}

fn hex(value: &str, size: usize) -> bool {
    value.len() == size
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn source_path(path: &str) -> bool {
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path.contains(':')
        || path.chars().any(char::is_control)
    {
        return false;
    }
    path.split('/').all(|part| {
        !part.is_empty() && part != "." && part != ".." && !part.eq_ignore_ascii_case(".git")
    })
}

impl SourceRef {
    fn validate(&self) -> Result<()> {
        require(
            !self.id.is_empty()
                && self.id.len() <= 64
                && self
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
                && source_path(&self.path)
                && self.start_line > 0
                && self.end_line >= self.start_line,
            "packet_source_ref",
        )
    }
}

fn validate_common<'a>(
    version: u32,
    revision: &str,
    task: &str,
    sources: impl Iterator<Item = &'a SourceRef>,
    source_count: usize,
) -> Result<()> {
    require(
        version == 1
            && hex(revision, 40)
            && !task.trim().is_empty()
            && task.len() <= MAX_TASK_BYTES
            && !task.contains('\0')
            && (1..=MAX_REFS).contains(&source_count),
        "packet_spec",
    )?;
    let mut ids = BTreeSet::new();
    for source in sources {
        source.validate()?;
        require(ids.insert(&source.id), "packet_duplicate_id")?;
    }
    Ok(())
}

impl Spec {
    pub fn validate(&self) -> Result<()> {
        validate_common(
            self.version,
            &self.source_revision,
            &self.task,
            self.required.iter().chain(&self.chunks),
            self.required.len() + self.chunks.len(),
        )
    }
}

impl Packet {
    pub fn validate(&self) -> Result<()> {
        validate_common(
            self.version,
            &self.source_revision,
            &self.task,
            self.required
                .iter()
                .chain(&self.chunks)
                .map(|entry| &entry.source),
            self.required.len() + self.chunks.len(),
        )?;
        for entry in self.required.iter().chain(&self.chunks) {
            require(
                hex(&entry.file_sha256, 64)
                    && hex(&entry.sha256, 64)
                    && !entry.text.is_empty()
                    && entry.text.len() <= MAX_SOURCE_BYTES
                    && !entry.text.contains('\0')
                    && entry.sha256 == sha256(entry.text.as_bytes()),
                "packet_excerpt",
            )?;
        }
        require(encoded(self)?.len() <= MAX_PACKET_BYTES, "packet_size")
    }

    pub fn digest(&self) -> Result<String> {
        self.validate()?;
        digest(self)
    }

    fn summary(&self, freshness: &str) -> Result<Summary> {
        Ok(Summary {
            packet_sha256: self.digest()?,
            freshness: freshness.into(),
            required_count: self.required.len(),
            chunk_count: self.chunks.len(),
            source_count: self
                .required
                .iter()
                .chain(&self.chunks)
                .map(|entry| &entry.source.path)
                .collect::<BTreeSet<_>>()
                .len(),
        })
    }

    /// Report a build without claiming current-checkout freshness.
    pub fn build_summary(&self) -> Result<Summary> {
        self.summary("not_checked")
    }

    /// Preserve every required and candidate excerpt in its original group.
    /// Call `verify` first when the projection will be used as current context.
    pub fn context_chunks(&self) -> (Vec<Chunk>, Vec<Chunk>) {
        let project = |entry: &SourceExcerpt| Chunk {
            id: entry.source.id.clone(),
            source: format!(
                "git:{}:{}#L{}-L{}",
                self.source_revision,
                entry.source.path,
                entry.source.start_line,
                entry.source.end_line
            ),
            text: entry.text.clone(),
            sha256: entry.sha256.clone(),
        };
        (
            self.required.iter().map(project).collect(),
            self.chunks.iter().map(project).collect(),
        )
    }

    fn spec(&self) -> Spec {
        Spec {
            version: self.version,
            source_revision: self.source_revision.clone(),
            task: self.task.clone(),
            required: self
                .required
                .iter()
                .map(|entry| entry.source.clone())
                .collect(),
            chunks: self
                .chunks
                .iter()
                .map(|entry| entry.source.clone())
                .collect(),
        }
    }
}

pub fn read_spec(path: &Path) -> Result<Spec> {
    let value = load(path, MAX_SPEC_BYTES)?;
    let spec: Spec = serde_json::from_value(value).map_err(|_| Stop::from("packet_spec_json"))?;
    spec.validate()?;
    Ok(spec)
}

pub fn read_packet(path: &Path) -> Result<Packet> {
    let value = load(path, MAX_PACKET_BYTES)?;
    let packet: Packet = serde_json::from_value(value).map_err(|_| Stop::from("packet_json"))?;
    packet.validate()?;
    Ok(packet)
}

pub fn write_packet(path: &Path, packet: &Packet) -> Result<()> {
    packet.validate()?;
    let mut bytes = serde_json::to_vec_pretty(packet).map_err(|_| Stop::from("packet_json"))?;
    bytes.push(b'\n');
    require(bytes.len() <= MAX_PACKET_BYTES, "packet_size")?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| Stop::from("packet_output_exists_or_unavailable"))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| Stop::from("packet_output_io"))
}

fn read_bounded(reader: impl Read, limit: usize, over: &AtomicBool) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        over.store(true, Ordering::SeqCst);
    }
    Ok(bytes)
}

fn git(repo: &Path, args: &[&str], limit: usize) -> Result<Vec<u8>> {
    run_git_command(repo, Path::new("git"), args, limit, GIT_TIMEOUT)
}

fn run_git_command(
    repo: &Path,
    program: &Path,
    args: &[&str],
    limit: usize,
    timeout: Duration,
) -> Result<Vec<u8>> {
    let mut command = Command::new(program);
    command
        .arg("--literal-pathspecs")
        .args(args)
        .current_dir(repo)
        .env_clear()
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|_| Stop::from("packet_git_spawn"))?;
    let exceeded = Arc::new(AtomicBool::new(false));
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Stop::from("packet_git_io"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| Stop::from("packet_git_io"))?;
    enum Stream {
        Output(std::io::Result<Vec<u8>>),
        Error(std::io::Result<Vec<u8>>),
    }
    let (sender, receiver) = mpsc::channel();
    let output_flag = exceeded.clone();
    let error_flag = exceeded.clone();
    let output_sender = sender.clone();
    let output_reader = thread::spawn(move || {
        let _ = output_sender.send(Stream::Output(read_bounded(stdout, limit, &output_flag)));
    });
    let error_reader = thread::spawn(move || {
        let _ = sender.send(Stream::Error(read_bounded(
            stderr,
            GIT_STDERR_BYTES,
            &error_flag,
        )));
    });
    let started = Instant::now();
    let (mut output, mut error, mut status) = (None, None, None);
    loop {
        if exceeded.load(Ordering::SeqCst) {
            stop_git(&mut child);
            return Err("packet_git_output_limit".into());
        }
        while let Ok(stream) = receiver.try_recv() {
            match stream {
                Stream::Output(bytes) => output = Some(bytes),
                Stream::Error(bytes) => error = Some(bytes),
            }
        }
        if status.is_none() {
            status = match child.try_wait() {
                Ok(value) => value,
                Err(_) => {
                    stop_git(&mut child);
                    return Err("packet_git_io".into());
                }
            };
        }
        if status.is_some() && output.is_some() && error.is_some() {
            break;
        }
        if started.elapsed() >= timeout {
            stop_git(&mut child);
            return Err("packet_git_timeout".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
    output_reader
        .join()
        .map_err(|_| Stop::from("packet_git_io"))?;
    error_reader
        .join()
        .map_err(|_| Stop::from("packet_git_io"))?;
    require(!exceeded.load(Ordering::SeqCst), "packet_git_output_limit")?;
    require(
        status.is_some_and(|status| status.success()),
        "packet_git_failed",
    )?;
    error
        .ok_or_else(|| Stop::from("packet_git_io"))?
        .map_err(|_| Stop::from("packet_git_io"))?;
    output
        .ok_or_else(|| Stop::from("packet_git_io"))?
        .map_err(|_| Stop::from("packet_git_io"))
}

fn stop_git(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        // Git has its own process group, so helpers cannot keep our pipes open.
        unsafe {
            libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn line(bytes: &[u8]) -> Result<&str> {
    let text = std::str::from_utf8(bytes).map_err(|_| Stop::from("packet_git_output"))?;
    Ok(text.trim_end_matches('\n').trim_end_matches('\r'))
}

fn repository(repo: &Path) -> Result<PathBuf> {
    let root = fs::canonicalize(repo).map_err(|_| Stop::from("packet_repo"))?;
    require(root.is_dir(), "packet_repo")?;
    let git_root = git(&root, &["rev-parse", "--show-toplevel"], 4096)?;
    let git_root = fs::canonicalize(line(&git_root)?).map_err(|_| Stop::from("packet_repo"))?;
    require(root == git_root, "packet_repo_root_required")?;
    Ok(root)
}

fn committed_revision(repo: &Path, revision: &str) -> Result<()> {
    require(hex(revision, 40), "packet_revision")?;
    let expression = format!("{revision}^{{commit}}");
    let result = git(repo, &["rev-parse", "--verify", &expression], 128)?;
    require(line(&result)? == revision, "packet_revision")
}

fn committed_file(repo: &Path, revision: &str, path: &str) -> Result<Vec<u8>> {
    let rows = git(
        repo,
        &["ls-tree", "-z", "-r", "--full-tree", revision, "--", path],
        4096,
    )?;
    let mut entries = rows.split(|byte| *byte == 0).filter(|row| !row.is_empty());
    let row = entries
        .next()
        .ok_or_else(|| Stop::from("packet_source_missing"))?;
    require(entries.next().is_none(), "packet_source_ambiguous")?;
    let (meta, name) = row
        .split_once_byte(b'\t')
        .ok_or_else(|| Stop::from("packet_git_output"))?;
    require(name == path.as_bytes(), "packet_source_missing")?;
    let meta = std::str::from_utf8(meta).map_err(|_| Stop::from("packet_git_output"))?;
    let mut fields = meta.split(' ');
    let mode = fields
        .next()
        .ok_or_else(|| Stop::from("packet_git_output"))?;
    let kind = fields
        .next()
        .ok_or_else(|| Stop::from("packet_git_output"))?;
    let object = fields
        .next()
        .ok_or_else(|| Stop::from("packet_git_output"))?;
    require(
        fields.next().is_none()
            && (mode == "100644" || mode == "100755")
            && kind == "blob"
            && hex(object, 40),
        "packet_source_not_regular",
    )?;
    let bytes = git(repo, &["cat-file", "blob", object], MAX_SOURCE_BYTES)?;
    require(
        !bytes.is_empty() && !bytes.contains(&0),
        "packet_source_not_text",
    )?;
    std::str::from_utf8(&bytes).map_err(|_| Stop::from("packet_source_not_text"))?;
    Ok(bytes)
}

trait SplitOnceByte {
    fn split_once_byte(&self, byte: u8) -> Option<(&[u8], &[u8])>;
}
impl SplitOnceByte for [u8] {
    fn split_once_byte(&self, byte: u8) -> Option<(&[u8], &[u8])> {
        let index = self.iter().position(|candidate| *candidate == byte)?;
        Some((&self[..index], &self[index + 1..]))
    }
}

fn excerpt(bytes: &[u8], source: &SourceRef) -> Result<String> {
    let mut line_start = 0;
    let mut first = None;
    let mut last = None;
    let mut number = 1;
    while line_start < bytes.len() {
        let next = bytes[line_start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |relative| line_start + relative + 1);
        if number == source.start_line {
            first = Some(line_start);
        }
        if number == source.end_line {
            last = Some(next);
            break;
        }
        number += 1;
        line_start = next;
    }
    let (start, end) = first
        .zip(last)
        .ok_or_else(|| Stop::from("packet_source_range"))?;
    let value = std::str::from_utf8(&bytes[start..end])
        .map_err(|_| Stop::from("packet_source_not_text"))?;
    require(!value.is_empty(), "packet_source_range")?;
    Ok(value.into())
}

fn build_at_root(root: &Path, spec: &Spec) -> Result<(Packet, BTreeMap<String, Vec<u8>>)> {
    spec.validate()?;
    committed_revision(root, &spec.source_revision)?;
    let mut files = BTreeMap::<String, Vec<u8>>::new();
    let mut total = 0usize;
    for source in spec.required.iter().chain(&spec.chunks) {
        if !files.contains_key(&source.path) {
            let bytes = committed_file(root, &spec.source_revision, &source.path)?;
            total = total
                .checked_add(bytes.len())
                .ok_or_else(|| Stop::from("packet_source_budget"))?;
            require(total <= MAX_TOTAL_SOURCE_BYTES, "packet_source_budget")?;
            files.insert(source.path.clone(), bytes);
        }
    }
    let capture = |source: &SourceRef| -> Result<SourceExcerpt> {
        let bytes = files
            .get(&source.path)
            .ok_or_else(|| Stop::from("packet_source_missing"))?;
        let text = excerpt(bytes, source)?;
        Ok(SourceExcerpt {
            source: source.clone(),
            file_sha256: sha256(bytes),
            sha256: sha256(text.as_bytes()),
            text,
        })
    };
    let packet = Packet {
        version: spec.version,
        source_revision: spec.source_revision.clone(),
        task: spec.task.clone(),
        required: spec.required.iter().map(capture).collect::<Result<_>>()?,
        chunks: spec.chunks.iter().map(capture).collect::<Result<_>>()?,
    };
    packet.validate()?;
    Ok((packet, files))
}

/// Capture the caller's entire selection from an immutable commit.
/// Build deliberately does not assert that the current checkout matches it.
pub fn build(repo: &Path, spec: &Spec) -> Result<Packet> {
    let root = repository(repo)?;
    Ok(build_at_root(&root, spec)?.0)
}

fn head(root: &Path) -> Result<String> {
    let output = git(root, &["rev-parse", "--verify", "HEAD^{commit}"], 128)?;
    let head = line(&output)?;
    require(hex(head, 40), "packet_head")?;
    Ok(head.into())
}

#[cfg(target_os = "linux")]
fn open_working_file(root: &Path, path: &str) -> Result<File> {
    use std::{
        ffi::CString,
        os::fd::{AsRawFd, FromRawFd},
    };

    let mut directory = File::open(root).map_err(|_| Stop::from("packet_stale_file"))?;
    let parts: Vec<_> = path.split('/').collect();
    for (index, part) in parts.iter().enumerate() {
        let part = CString::new(*part).map_err(|_| Stop::from("packet_stale_file"))?;
        let is_file = index + 1 == parts.len();
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | if is_file {
                libc::O_NONBLOCK
            } else {
                libc::O_DIRECTORY
            };
        // Each component is opened relative to the preceding directory handle.
        // O_NOFOLLOW prevents a concurrent symlink swap from redirecting the read.
        let descriptor = unsafe { libc::openat(directory.as_raw_fd(), part.as_ptr(), flags) };
        require(descriptor >= 0, "packet_stale_file")?;
        // openat returned an owned descriptor.
        let opened = unsafe { File::from_raw_fd(descriptor) };
        let meta = opened
            .metadata()
            .map_err(|_| Stop::from("packet_stale_file"))?;
        require(
            if is_file {
                meta.is_file()
            } else {
                meta.is_dir()
            },
            "packet_stale_file",
        )?;
        directory = opened;
    }
    Ok(directory)
}

#[cfg(not(target_os = "linux"))]
fn open_working_file(root: &Path, path: &str) -> Result<File> {
    let mut current = root.to_path_buf();
    let parts: Vec<_> = path.split('/').collect();
    for (index, part) in parts.iter().enumerate() {
        current.push(part);
        let meta = fs::symlink_metadata(&current).map_err(|_| Stop::from("packet_stale_file"))?;
        require(
            !meta.file_type().is_symlink()
                && if index + 1 == parts.len() {
                    meta.is_file()
                } else {
                    meta.is_dir()
                },
            "packet_stale_file",
        )?;
    }
    File::open(current).map_err(|_| Stop::from("packet_stale_file"))
}

fn working_file(root: &Path, path: &str) -> Result<Vec<u8>> {
    let file = open_working_file(root, path)?;
    let mut bytes = Vec::new();
    file.take(MAX_SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Stop::from("packet_stale_file"))?;
    require(bytes.len() <= MAX_SOURCE_BYTES, "packet_stale_file")?;
    Ok(bytes)
}

/// Rebuild from pinned blobs and check current HEAD and full working-file bytes.
pub fn verify(repo: &Path, packet: &Packet) -> Result<Summary> {
    packet.validate()?;
    let root = repository(repo)?;
    require(head(&root)? == packet.source_revision, "packet_stale_head")?;
    let (rebuilt, files) = build_at_root(&root, &packet.spec())?;
    require(&rebuilt == packet, "packet_tampered")?;
    for (path, expected) in files {
        require(working_file(&root, &path)? == expected, "packet_stale_file")?;
    }
    require(head(&root)? == packet.source_revision, "packet_stale_head")?;
    packet.summary("current")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn git_timeout_covers_descendants_that_keep_pipes_open() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-git");
        let marker = dir.path().join("survived");
        fs::write(
            &script,
            b"#!/bin/sh\n( /bin/sleep 0.3; printf survived > \"$2\" ) &\nexit 0\n",
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let marker_arg = marker.to_str().unwrap();
        let started = Instant::now();
        assert_eq!(
            run_git_command(
                dir.path(),
                &script,
                &[marker_arg],
                128,
                Duration::from_millis(50)
            )
            .unwrap_err()
            .0,
            "packet_git_timeout"
        );
        assert!(started.elapsed() < Duration::from_secs(1));
        thread::sleep(Duration::from_millis(400));
        assert!(!marker.exists(), "timed-out descendant was not killed");
    }
}
