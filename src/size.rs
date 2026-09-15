use std::os::unix::fs::{MetadataExt, DirEntryExt};
use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
};

use crate::error::Error;

pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    // Use the below to align with du -s. 
    //format!("{:.2}", (bytes as f64 / KB as f64) as u64)

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.2} KB", bytes)
    }
}

struct TaskQueue {
    queue: std::sync::Mutex<VecDeque<PathBuf>>,
    condvar: std::sync::Condvar,
    finished: std::sync::Mutex<bool>,
}

impl TaskQueue {
    fn new() -> Self {
        Self {
            queue: std::sync::Mutex::new(VecDeque::new()),
            condvar: std::sync::Condvar::new(),
            finished: std::sync::Mutex::new(false),
        }
    }

    fn push(&self, task: PathBuf) {
        let mut q = self.queue.lock().unwrap();
        q.push_back(task);
        self.condvar.notify_one();
    }

    fn set_finished(&self) {
        let mut finished = self.finished.lock().unwrap();
        *finished = true;
        self.condvar.notify_all();
    }

    fn pop(&self) -> Option<PathBuf> {
        let mut q = self.queue.lock().unwrap();
        loop {
            if let Some(task) = q.pop_front() {
                return Some(task);
            }

            let finished = *self.finished.lock().unwrap();
            if finished && q.is_empty() {
                return None;
            }

            q = self.condvar.wait(q).unwrap();
        }
    }
}

pub fn orchestrate_directories_size_calculation() -> Result<(), Error> {
    let snapshot_dir = Path::new("/var/snaps/snapshots/arch-theo/");
    let mut snapshots: Vec<PathBuf> = snapshot_dir
        .read_dir()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .map(|entry| entry.path())
        .collect();

    snapshots.sort();

    let task_queue = std::sync::Arc::new(TaskQueue::new());

    for snap in snapshots {
        task_queue.push(snap);
    }

    task_queue.set_finished();

    let num_workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let mut handles = vec![];

    for _ in 0..num_workers {
        let queue_clone = std::sync::Arc::clone(&task_queue);

        let handle = std::thread::spawn(move || -> Result<Vec<(PathBuf, u64, u64)>, Error> {
            let mut results = Vec::new();
            while let Some(task) = queue_clone.pop() {
                //let (uniq, hl) = compute_dir_entry_size(&task)?;
                let sizes = linear_directory_size(&task)?;
                let uniq = (sizes.st_blocks - sizes.shared_st_blocks) * 512;
                let hl = sizes.shared_st_blocks * 512;
                results.push((task, uniq, hl));
            }

            Ok(results)
        });
        handles.push(handle);
    }

    let mut snapshot_sizes = Vec::new();
    for handle in handles {
        match handle.join() {
            Ok(thread_result) => {
                snapshot_sizes.extend(thread_result?);
            }
            Err(e) => std::panic::resume_unwind(e),
        }
    }
    snapshot_sizes.sort_unstable_by(|a, b| a.0.cmp(&b.0));

    println!(
        "{:<12} | {:<20} | {:<10} | {:<10} | {:<10}",
        "Host", "Snapshot", "Unique", "Hard-link", "Total"
    );
    for (snap, uniq_size, hl_size) in snapshot_sizes {
        let parent = snap
            .parent()
            .and_then(|p| p.file_name().unwrap().to_str())
            .unwrap();
        let snapshot = snap.file_name().and_then(|p| p.to_str()).unwrap();

        println!(
            "{:<12} | {:<20} | {:<10} | {:<10} | {:<10}",
            parent,
            snapshot,
            format_size(uniq_size),
            format_size(hl_size),
            format_size(uniq_size + hl_size),
        );
    }
    //let total_size: u64 = global_hashmap.lock().unwrap().values().sum();
    //let fmt_total_size = format_size(total_size);
    //println!("Total size: {}", fmt_total_size);

    Ok(())
}


pub struct DirectorySize {
    st_size: u64,
    st_blocks: u64,
    shared_st_size: u64,
    shared_st_blocks: u64,
}

impl std::fmt::Display for DirectorySize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fmt_st_size = format_size(self.st_size);
        let fmt_st_blocks = format_size(self.st_blocks * 512);
        let fmt_shared_st_size = format_size(self.shared_st_size);
        let fmt_shared_st_blocks = format_size(self.shared_st_blocks * 512);

        writeln!(f, "st_size: {}, st_blocks: {}, shared st_size: {}, shared st_blocks: {}",
            fmt_st_size,
            fmt_st_blocks,
            fmt_shared_st_size,
            fmt_shared_st_blocks)
    }
}

pub fn linear_directory_size(path: &Path) -> Result<DirectorySize, Error> {
    let mut inode_history = HashSet::with_capacity(10_000);

    let mut stack = vec![path.to_path_buf()];


    let mut st_size = 0;
    let mut st_blocks = 0;
    let mut shared_st_size = 0;
    let mut shared_st_blocks = 0;
    
    // Don't forget to account for the dir
    // that was passed as argument.
    let path_metadata = std::fs::symlink_metadata(path)?;
    let path_size = path_metadata.len();
    let path_blocks = path_metadata.blocks();
    
    st_size += path_size;
    st_blocks += path_blocks;

    while let Some(path) = stack.pop() {
        let entries = match std::fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(_) => {
                eprintln!("Unable to read path: {:?}", &path);
                continue;
            },
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            
            if file_type.is_symlink() {
                continue;
            }

            let entry_metadata = entry.metadata()?;

            let nlink = entry_metadata.nlink();
            
            if entry_metadata.is_file() && nlink > 1 {
                let dev = entry_metadata.dev();
                let inode = entry.ino();
                
                if !inode_history.insert((dev, inode)) {
                    continue;
                }
                
                shared_st_size += entry_metadata.len();
                shared_st_blocks += entry_metadata.blocks();
            } 
            
            st_size += entry_metadata.len();
            st_blocks += entry_metadata.blocks();

            if file_type.is_dir() {
                stack.push(entry.path()); 
            }
        }
    }
    
    Ok(DirectorySize {
        st_size,
        st_blocks,
        shared_st_size,
        shared_st_blocks
    })
}
