use crate::model::ProcessSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize)]
struct CpuCacheFile {
    timestamp_ms: u64,
    samples: Vec<CpuSample>,
}

#[derive(Serialize, Deserialize)]
struct CpuSample {
    pid: u32,
    start_time_seconds: u64,
    accumulated_cpu_time_ms: u64,
}

pub fn estimate_cpu_usage_and_store(
    snapshots: &HashMap<u32, ProcessSnapshot>,
) -> HashMap<u32, f32> {
    let now_ms = now_ms();
    let previous = load_cache();
    let mut previous_by_key = previous
        .as_ref()
        .map(|cache| {
            cache
                .samples
                .iter()
                .map(|sample| {
                    (
                        (sample.pid, sample.start_time_seconds),
                        sample.accumulated_cpu_time_ms,
                    )
                })
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();

    let elapsed_ms = previous
        .as_ref()
        .map(|cache| now_ms.saturating_sub(cache.timestamp_ms))
        .unwrap_or(0);

    let usage = snapshots
        .values()
        .map(|snapshot| {
            let cpu_usage = if elapsed_ms >= 50 && snapshot.start_time_seconds != 0 {
                previous_by_key
                    .remove(&(snapshot.pid, snapshot.start_time_seconds))
                    .map(|previous_cpu_ms| {
                        snapshot
                            .accumulated_cpu_time_ms
                            .saturating_sub(previous_cpu_ms) as f64
                            / elapsed_ms as f64
                            * 100.0
                    })
                    .unwrap_or(0.0)
            } else {
                0.0
            };

            (snapshot.pid, cpu_usage.clamp(0.0, 10_000.0) as f32)
        })
        .collect::<HashMap<_, _>>();

    let next = CpuCacheFile {
        timestamp_ms: now_ms,
        samples: snapshots
            .values()
            .filter(|snapshot| snapshot.start_time_seconds != 0)
            .map(|snapshot| CpuSample {
                pid: snapshot.pid,
                start_time_seconds: snapshot.start_time_seconds,
                accumulated_cpu_time_ms: snapshot.accumulated_cpu_time_ms,
            })
            .collect(),
    };
    save_cache(&next);

    usage
}

fn load_cache() -> Option<CpuCacheFile> {
    let path = cache_path();
    let contents = fs::read(&path).ok()?;
    serde_json::from_slice::<CpuCacheFile>(&contents).ok()
}

fn save_cache(cache: &CpuCacheFile) {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(contents) = serde_json::to_vec(cache) {
        let _ = fs::write(path, contents);
    }
}

fn cache_path() -> PathBuf {
    std::env::temp_dir()
        .join("ports-cli")
        .join("cpu-cache.json")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
