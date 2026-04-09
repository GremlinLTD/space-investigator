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
