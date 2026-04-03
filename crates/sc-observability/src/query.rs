use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use sc_observability_types::{
    ErrorContext, LogEvent, LogOrder, LogQuery, LogSnapshot, QueryError, Remediation, error_codes,
};
use serde_json::{Value, json};

#[cfg(test)]
use crate::rotated_log_path;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct FileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(not(unix))]
    len: u64,
    #[cfg(not(unix))]
    modified_nanos: Option<u128>,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedLogFile {
    pub(crate) path: PathBuf,
    pub(crate) identity: FileIdentity,
    pub(crate) len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrackedFile {
    pub(crate) path: PathBuf,
    pub(crate) identity: FileIdentity,
    pub(crate) offset: u64,
}

pub(crate) fn query_snapshot(
    active_log_path: &Path,
    query: &LogQuery,
) -> Result<LogSnapshot, QueryError> {
    query.validate()?;
    let resolved = resolve_visible_files(active_log_path)?;
    read_snapshot(query, &resolved, |_file| 0)
}

pub(crate) fn start_follow_tracking(
    active_log_path: &Path,
) -> Result<Vec<TrackedFile>, QueryError> {
    Ok(resolve_visible_files(active_log_path)?
        .into_iter()
        .map(|file| TrackedFile {
            path: file.path,
            identity: file.identity,
            offset: file.len,
        })
        .collect())
}

pub(crate) fn poll_follow_snapshot(
    active_log_path: &Path,
    query: &LogQuery,
    tracked_files: &mut Vec<TrackedFile>,
) -> Result<LogSnapshot, QueryError> {
    let resolved = resolve_visible_files(active_log_path)?;
    let previous = std::mem::take(tracked_files);
    *tracked_files = resolved
        .iter()
        .map(|file| TrackedFile {
            path: file.path.clone(),
            identity: file.identity.clone(),
            offset: tracked_offset_for(file, &previous),
        })
        .collect();

    read_incremental_snapshot(query, &resolved, tracked_files)
}

fn tracked_offset_for(file: &ResolvedLogFile, previous: &[TrackedFile]) -> u64 {
    if let Some(tracked) = previous.iter().find(|tracked| tracked.path == file.path) {
        if tracked.identity != file.identity {
            return 0;
        }
        return if tracked.offset <= file.len {
            tracked.offset
        } else {
            0
        };
    }

    previous
        .iter()
        .find(|tracked| tracked.identity == file.identity)
        .map(|tracked| {
            if tracked.offset <= file.len {
                tracked.offset
            } else {
                0
            }
        })
        .unwrap_or(0)
}

pub(crate) fn shutdown_error() -> QueryError {
    QueryError::Shutdown
}

pub(crate) fn unavailable_error(message: impl Into<String>) -> QueryError {
    QueryError::Unavailable(Box::new(ErrorContext::new(
        error_codes::SC_LOG_QUERY_UNAVAILABLE,
        message,
        Remediation::recoverable("enable or restore the JSONL log source", ["retry"]),
    )))
}

fn io_error(
    path: &Path,
    action: &str,
    error: impl std::error::Error + Send + Sync + 'static,
) -> QueryError {
    let rendered_path = path.display().to_string();
    let cause = error.to_string();
    QueryError::Io(Box::new(
        ErrorContext::new(
            error_codes::SC_LOG_QUERY_IO,
            format!("failed to {action} log file"),
            Remediation::recoverable("verify the log path and file permissions", ["retry"]),
        )
        .cause(cause)
        .detail("path", Value::String(rendered_path))
        .source(Box::new(error)),
    ))
}

fn decode_error(
    path: &Path,
    offset: u64,
    line: &str,
    error: impl std::error::Error + Send + Sync + 'static,
) -> QueryError {
    let cause = error.to_string();
    QueryError::Decode(Box::new(
        ErrorContext::new(
            error_codes::SC_LOG_QUERY_DECODE,
            "failed to decode JSONL log record",
            Remediation::recoverable(
                "repair or remove the malformed JSONL record",
                ["retry the query"],
            ),
        )
        .cause(cause)
        .detail("path", Value::String(path.display().to_string()))
        .detail("offset", json!(offset))
        .detail("line", Value::String(line.to_string()))
        .source(Box::new(error)),
    ))
}

fn resolve_visible_files(active_log_path: &Path) -> Result<Vec<ResolvedLogFile>, QueryError> {
    let parent = active_log_path.parent().unwrap_or_else(|| Path::new("."));
    let active_name = active_log_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("active.log.jsonl")
        .to_string();

    let mut rotated = Vec::new();
    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries {
            let entry = entry.map_err(|err| io_error(parent, "read log directory entry", err))?;
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let Some(suffix) = file_name.strip_prefix(&format!("{active_name}.")) else {
                continue;
            };
            let Ok(index) = suffix.parse::<u32>() else {
                continue;
            };
            let metadata = entry
                .metadata()
                .map_err(|err| io_error(&path, "read log file metadata", err))?;
            rotated.push((
                index,
                ResolvedLogFile {
                    path,
                    identity: file_identity(&metadata),
                    len: metadata.len(),
                },
            ));
        }
    }

    rotated.sort_by(|left, right| right.0.cmp(&left.0));
    let mut resolved: Vec<ResolvedLogFile> = rotated.into_iter().map(|(_, file)| file).collect();

    if let Ok(metadata) = fs::metadata(active_log_path) {
        resolved.push(ResolvedLogFile {
            path: active_log_path.to_path_buf(),
            identity: file_identity(&metadata),
            len: metadata.len(),
        });
    }

    Ok(resolved)
}

fn read_snapshot(
    query: &LogQuery,
    resolved: &[ResolvedLogFile],
    offset_for: impl Fn(&ResolvedLogFile) -> u64,
) -> Result<LogSnapshot, QueryError> {
    let mut matches = Vec::new();

    for file in resolved {
        let (events, _) = read_events_from_path(&file.path, offset_for(file))?;
        for event in events {
            if event_matches_query(&event, query) {
                matches.push(event);
            }
        }
    }

    Ok(finalize_snapshot(query, matches))
}

fn read_incremental_snapshot(
    query: &LogQuery,
    resolved: &[ResolvedLogFile],
    tracked_files: &mut [TrackedFile],
) -> Result<LogSnapshot, QueryError> {
    let mut matches = Vec::new();

    for (file, tracked) in resolved.iter().zip(tracked_files.iter_mut()) {
        let (events, end_offset) = read_events_from_path(&file.path, tracked.offset)?;
        tracked.offset = end_offset;
        for event in events {
            if event_matches_query(&event, query) {
                matches.push(event);
            }
        }
    }

    Ok(finalize_snapshot(query, matches))
}

fn finalize_snapshot(query: &LogQuery, mut matches: Vec<LogEvent>) -> LogSnapshot {
    if matches!(query.order, LogOrder::NewestFirst) {
        matches.reverse();
    }

    let truncated = query.limit.is_some_and(|limit| matches.len() > limit);

    if let Some(limit) = query.limit {
        matches.truncate(limit);
    }

    LogSnapshot {
        events: matches,
        truncated,
    }
}

fn read_events_from_path(
    path: &Path,
    start_offset: u64,
) -> Result<(Vec<LogEvent>, u64), QueryError> {
    let file = File::open(path).map_err(|err| io_error(path, "open", err))?;
    let file_len = file
        .metadata()
        .map_err(|err| io_error(path, "read log file metadata", err))?
        .len();
    let start_offset = start_offset.min(file_len);

    let mut reader = BufReader::new(file);
    reader
        .seek(SeekFrom::Start(start_offset))
        .map_err(|err| io_error(path, "seek within", err))?;

    let mut events = Vec::new();
    let mut line = String::new();
    loop {
        let line_offset = reader
            .stream_position()
            .map_err(|err| io_error(path, "read stream position", err))?;
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|err| io_error(path, "read", err))?;
        if bytes == 0 {
            break;
        }
        let raw = line.trim_end_matches('\n').trim_end_matches('\r');
        let event = serde_json::from_str::<LogEvent>(raw)
            .map_err(|err| decode_error(path, line_offset, raw, err))?;
        events.push(event);
    }

    let end_offset = reader
        .stream_position()
        .map_err(|err| io_error(path, "read stream position", err))?;
    Ok((events, end_offset))
}

fn event_matches_query(event: &LogEvent, query: &LogQuery) -> bool {
    query
        .service
        .as_ref()
        .is_none_or(|service| &event.service == service)
        && (query.levels.is_empty() || query.levels.contains(&event.level))
        && query
            .target
            .as_ref()
            .is_none_or(|target| &event.target == target)
        && query
            .action
            .as_ref()
            .is_none_or(|action| &event.action == action)
        && query
            .request_id
            .as_ref()
            .is_none_or(|request_id| event.request_id.as_ref() == Some(request_id))
        && query
            .correlation_id
            .as_ref()
            .is_none_or(|correlation_id| event.correlation_id.as_ref() == Some(correlation_id))
        && query.since.is_none_or(|since| event.timestamp >= since)
        && query.until.is_none_or(|until| event.timestamp <= until)
        && query.field_matches.iter().all(|field_match| {
            event
                .fields
                .get(&field_match.field)
                .is_some_and(|value| value == &field_match.value)
        })
}

fn file_identity(metadata: &fs::Metadata) -> FileIdentity {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }

    #[cfg(not(unix))]
    {
        let modified_nanos = metadata
            .modified()
            .ok()
            .and_then(|modified| {
                modified
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .ok()
            })
            .map(|duration| duration.as_nanos());
        FileIdentity {
            len: metadata.len(),
            modified_nanos,
        }
    }
}

#[cfg(test)]
pub(crate) fn query_active_and_rotated_paths(active_path: &Path, max_files: u32) -> Vec<PathBuf> {
    let mut paths = (1..=max_files)
        .rev()
        .map(|index| rotated_log_path(active_path, index))
        .collect::<Vec<_>>();
    paths.push(active_path.to_path_buf());
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_identity(seed: u64) -> FileIdentity {
        #[cfg(unix)]
        {
            FileIdentity {
                device: 1,
                inode: seed,
            }
        }

        #[cfg(not(unix))]
        {
            FileIdentity {
                len: seed,
                modified_nanos: Some(u128::from(seed)),
            }
        }
    }

    #[test]
    fn follow_tracking_resets_when_same_path_identity_changes() {
        let previous = vec![TrackedFile {
            path: PathBuf::from("active.log.jsonl"),
            identity: test_identity(1),
            offset: 128,
        }];
        let recreated = ResolvedLogFile {
            path: PathBuf::from("active.log.jsonl"),
            identity: test_identity(2),
            len: 512,
        };

        assert_eq!(tracked_offset_for(&recreated, &previous), 0);
    }

    #[test]
    fn follow_tracking_preserves_offset_when_rotation_moves_identity_to_new_path() {
        let previous = vec![TrackedFile {
            path: PathBuf::from("active.log.jsonl"),
            identity: test_identity(7),
            offset: 256,
        }];
        let rotated = ResolvedLogFile {
            path: PathBuf::from("active.log.jsonl.1"),
            identity: test_identity(7),
            len: 512,
        };

        assert_eq!(tracked_offset_for(&rotated, &previous), 256);
    }
}
