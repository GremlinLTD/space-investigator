use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Local;
use clap::Parser;
use jwalk::WalkDir;
use serde::Serialize;
use sysinfo::Disks;

#[derive(Parser)]
#[command(name = "si", about = "Parallel disk space analyzer", version)]
struct Args {
    /// Path to investigate (defaults to current directory)
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Number of top directories to show
    #[arg(short = 'd', long, default_value = "20")]
    dirs: usize,

    /// Number of top files to show
    #[arg(short = 'f', long, default_value = "20")]
    files: usize,

    /// Output as JSON
    #[arg(short, long)]
    json: bool,
}

struct SizeEntry {
    size_bytes: u64,
    path: PathBuf,
}

#[derive(Serialize)]
struct Report {
    timestamp: String,
    path: String,
    filesystem: Option<FilesystemInfo>,
    largest_directories: Vec<EntryInfo>,
    largest_files: Vec<EntryInfo>,
}

#[derive(Serialize)]
struct FilesystemInfo {
    name: String,
    mount_point: String,
    total_bytes: u64,
    used_bytes: u64,
    available_bytes: u64,
    use_percent: u64,
}

#[derive(Serialize)]
struct EntryInfo {
    path: String,
    size_bytes: u64,
    size_mb: u64,
}

fn main() {
    let args = Args::parse();
    let path = match args.path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: cannot access '{}': {}", args.path.display(), e);
            std::process::exit(1);
        }
    };

    let (dir_sizes, file_sizes) = collect_sizes(&path);

    if args.json {
        print_json(&path, dir_sizes, args.dirs, file_sizes, args.files);
    } else {
        print_text(&path, dir_sizes, args.dirs, file_sizes, args.files);
    }
}

fn print_text(
    path: &Path,
    dir_sizes: Vec<SizeEntry>,
    dir_count: usize,
    file_sizes: Vec<SizeEntry>,
    file_count: usize,
) {
    println!("{}", Local::now().format("%a %b %e %H:%M:%S %Z %Y"));
    print_disk_info(path);
    println!();
    print_largest("Largest Directories:", dir_sizes, dir_count);
    println!();
    print_largest("Largest Files:", file_sizes, file_count);
}

fn print_json(
    path: &Path,
    dir_sizes: Vec<SizeEntry>,
    dir_count: usize,
    file_sizes: Vec<SizeEntry>,
    file_count: usize,
) {
    let report = Report {
        timestamp: Local::now().to_rfc3339(),
        path: path.display().to_string(),
        filesystem: get_filesystem_info(path),
        largest_directories: dir_sizes
            .into_iter()
            .take(dir_count)
            .map(|e| EntryInfo {
                path: e.path.display().to_string(),
                size_bytes: e.size_bytes,
                size_mb: e.size_bytes / (1024 * 1024),
            })
            .collect(),
        largest_files: file_sizes
            .into_iter()
            .take(file_count)
            .map(|e| EntryInfo {
                path: e.path.display().to_string(),
                size_bytes: e.size_bytes,
                size_mb: e.size_bytes / (1024 * 1024),
            })
            .collect(),
    };

    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

fn get_filesystem_info(path: &Path) -> Option<FilesystemInfo> {
    let disks = Disks::new_with_refreshed_list();

    let disk = disks
        .iter()
        .filter(|d| path.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())?;

    let total = disk.total_space();
    let available = disk.available_space();
    let used = total.saturating_sub(available);
    let use_pct = if total > 0 {
        (used as f64 / total as f64 * 100.0) as u64
    } else {
        0
    };

    Some(FilesystemInfo {
        name: disk.name().to_string_lossy().into_owned(),
        mount_point: disk.mount_point().display().to_string(),
        total_bytes: total,
        used_bytes: used,
        available_bytes: available,
        use_percent: use_pct,
    })
}

fn print_disk_info(path: &Path) {
    if let Some(fs) = get_filesystem_info(path) {
        println!("Filesystem      Size  Used Avail Use% Mounted on");
        println!(
            "{:<15} {:>5} {:>5} {:>5} {:>3}% {}",
            fs.name,
            format_size(fs.total_bytes),
            format_size(fs.used_bytes),
            format_size(fs.available_bytes),
            fs.use_percent,
            fs.mount_point,
        );
    } else {
        eprintln!("Could not determine filesystem for {}", path.display());
    }
}

fn collect_sizes(root: &Path) -> (Vec<SizeEntry>, Vec<SizeEntry>) {
    let mut dir_sizes: HashMap<PathBuf, u64> = HashMap::new();
    let mut file_sizes: Vec<SizeEntry> = Vec::new();

    let root_dev = get_device_id(root);

    for entry in WalkDir::new(root)
        .skip_hidden(false)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
        let path = entry.path();

        if let Some(root_dev) = root_dev
            && let Some(entry_dev) = get_device_id(&path)
            && entry_dev != root_dev
        {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        if metadata.is_file() {
            let size = metadata.len();
            file_sizes.push(SizeEntry {
                size_bytes: size,
                path: path.clone(),
            });

            let mut parent = path.parent();
            while let Some(p) = parent {
                *dir_sizes.entry(p.to_path_buf()).or_default() += size;
                if p == root {
                    break;
                }
                parent = p.parent();
            }
        }
    }

    let mut dir_vec: Vec<SizeEntry> = dir_sizes
        .into_iter()
        .map(|(path, size_bytes)| SizeEntry { size_bytes, path })
        .collect();

    dir_vec.sort_unstable_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    file_sizes.sort_unstable_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

    (dir_vec, file_sizes)
}

fn print_largest(header: &str, entries: Vec<SizeEntry>, top: usize) {
    println!("{header}");
    for entry in entries.into_iter().take(top) {
        let mb = entry.size_bytes / (1024 * 1024);
        println!("{:>8} MB\t{}", mb, entry.path.display());
    }
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[(u64, &str)] = &[
        (1 << 50, "P"),
        (1 << 40, "T"),
        (1 << 30, "G"),
        (1 << 20, "M"),
        (1 << 10, "K"),
    ];

    for &(threshold, suffix) in UNITS {
        if bytes >= threshold {
            return format!("{:.1}{suffix}", bytes as f64 / threshold as f64);
        }
    }
    format!("{bytes}B")
}

#[cfg(unix)]
fn get_device_id(path: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).ok().map(|m| m.dev())
}

#[cfg(not(unix))]
fn get_device_id(_path: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn format_size_bytes() {
        assert_eq!(format_size(0), "0B");
        assert_eq!(format_size(512), "512B");
        assert_eq!(format_size(1023), "1023B");
    }

    #[test]
    fn format_size_kib() {
        assert_eq!(format_size(1024), "1.0K");
        assert_eq!(format_size(1536), "1.5K");
        assert_eq!(format_size(10240), "10.0K");
    }

    #[test]
    fn format_size_mib() {
        assert_eq!(format_size(1024 * 1024), "1.0M");
        assert_eq!(format_size(500 * 1024 * 1024), "500.0M");
    }

    #[test]
    fn format_size_gib() {
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0G");
        assert_eq!(format_size(2_u64 * 1024 * 1024 * 1024), "2.0G");
    }

    #[test]
    fn format_size_tib() {
        assert_eq!(format_size(1_u64 << 40), "1.0T");
    }

    #[test]
    fn format_size_pib() {
        assert_eq!(format_size(1_u64 << 50), "1.0P");
    }

    #[test]
    fn collect_sizes_on_temp_dir() {
        let dir = tempdir();
        fs::write(dir.join("a.txt"), "hello").unwrap();
        fs::write(dir.join("b.txt"), "hello world!").unwrap();
        fs::create_dir(dir.join("sub")).unwrap();
        fs::write(dir.join("sub/c.txt"), "test").unwrap();

        let (dirs, files) = collect_sizes(&dir);

        assert_eq!(files.len(), 3);
        assert_eq!(dirs.len(), 2); // dir and dir/sub

        // files sorted largest first
        assert!(files[0].size_bytes >= files[1].size_bytes);
        assert!(files[1].size_bytes >= files[2].size_bytes);

        // root dir contains all bytes
        let root_entry = dirs.iter().find(|e| e.path == dir).unwrap();
        let total: u64 = files.iter().map(|f| f.size_bytes).sum();
        assert_eq!(root_entry.size_bytes, total);
    }

    #[test]
    fn collect_sizes_empty_dir() {
        let dir = tempdir();
        let (dirs, files) = collect_sizes(&dir);
        assert!(files.is_empty());
        assert!(dirs.is_empty());
    }

    #[test]
    fn collect_sizes_respects_top_limit() {
        let dir = tempdir();
        for i in 0..10 {
            fs::write(dir.join(format!("{i}.txt")), format!("{i}")).unwrap();
        }

        let (_, files) = collect_sizes(&dir);
        let top_3: Vec<_> = files.into_iter().take(3).collect();
        assert_eq!(top_3.len(), 3);
    }

    #[cfg(unix)]
    #[test]
    fn get_device_id_returns_some() {
        let id = get_device_id(Path::new("."));
        assert!(id.is_some());
    }

    #[test]
    fn get_filesystem_info_for_current_dir() {
        let path = std::env::current_dir().unwrap();
        let info = get_filesystem_info(&path);
        assert!(info.is_some());
        let info = info.unwrap();
        assert!(info.total_bytes > 0);
        assert!(info.use_percent <= 100);
    }

    #[test]
    fn json_output_parses() {
        let dir = tempdir();
        fs::write(dir.join("test.txt"), "data").unwrap();

        let (dir_sizes, file_sizes) = collect_sizes(&dir);

        let report = Report {
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            path: dir.display().to_string(),
            filesystem: None,
            largest_directories: dir_sizes
                .into_iter()
                .take(5)
                .map(|e| EntryInfo {
                    path: e.path.display().to_string(),
                    size_bytes: e.size_bytes,
                    size_mb: e.size_bytes / (1024 * 1024),
                })
                .collect(),
            largest_files: file_sizes
                .into_iter()
                .take(5)
                .map(|e| EntryInfo {
                    path: e.path.display().to_string(),
                    size_bytes: e.size_bytes,
                    size_mb: e.size_bytes / (1024 * 1024),
                })
                .collect(),
        };

        let json = serde_json::to_string_pretty(&report).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["timestamp"], "2026-01-01T00:00:00Z");
        assert!(parsed["largest_files"].as_array().unwrap().len() == 1);
    }

    use std::sync::atomic::{AtomicU64, Ordering};

    fn tempdir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("si-test-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
