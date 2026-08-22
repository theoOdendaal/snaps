use std::{collections::{HashMap, HashSet, VecDeque}, path::{Path, PathBuf}, sync::Mutex};
use std::os::unix::fs::MetadataExt;

use crate::error::Error;

fn format_size(kilobytes: u64) -> String {
    //const KB: u64 = 1024;
    //const MB: u64 = KB * 1024;
    const MB: u64 = 1024;
    const GB: u64 = MB * 1024;

    if kilobytes >= GB {
        format!("{:.2} GB", kilobytes as f64 / GB as f64)
    } else if kilobytes >= MB {
        format!("{:.2} MB", kilobytes as f64 / MB as f64)
    } else {
        format!("{:.2} KB", kilobytes)
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
                return Some(task)
            }

            let finished = *self.finished.lock().unwrap();
            if finished && q.is_empty() {
                return None;
            }

            q = self.condvar.wait(q).unwrap();
        }

    }
}

fn compute_dir_entry_size(source_path: &Path, global_seen: &std::sync::Arc<Mutex<HashMap<(u64, u64), u64>>>) -> Result<(u64, u64), Error> {
    let mut seen_inodes = HashSet::with_capacity(8192);
    let mut stack: Vec<PathBuf> = Vec::with_capacity(8192);
    
    stack.push(source_path.to_path_buf());

    let mut total_uniq_blocks: u64 = 0;
    let mut total_hl_blocks: u64 = 0;

    while let Some(path) = stack.pop() {
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if metadata.file_type().is_symlink() {
            continue;
        }

        let file_id = (metadata.dev(), metadata.ino());

        let blocks = {
            let mut map = global_seen.lock().unwrap();
            *map.entry(file_id).or_insert_with(|| metadata.blocks())
        };

        if seen_inodes.insert(file_id) {
            if metadata.nlink() > 1 { 
                total_hl_blocks += blocks;
            } else {
                total_uniq_blocks += blocks;
            }
        }

        if metadata.is_dir() {
            let entries = match std::fs::read_dir(&path) {
                Ok(entries) => entries,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let mut child_path = PathBuf::with_capacity(path.as_os_str().len() + 64);
                child_path.push(&path);
                child_path.push(entry.file_name());
                stack.push(child_path);
            }
        }
    }
   
    // du presents sizes in units of 1024 byte, i.e. 1 Kilobyte
    Ok((total_uniq_blocks / 2,  total_hl_blocks / 2))
    //Ok(total_blocks * 512)
}

pub fn compute_par_snapshot_sizes() -> Result<(), Error> {

    let snapshot_dir = Path::new("/var/snaps/snapshots/arch-theo/");
    let mut snapshots: Vec<PathBuf> = snapshot_dir
        .read_dir()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .map(|entry| entry.path())
        .collect();

    snapshots.sort();

    let global_hashmap = std::sync::Arc::new(std::sync::Mutex::new(HashMap::with_capacity(65536)));
    
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

        let global_hashmap_clone = std::sync::Arc::clone(&global_hashmap);

        let handle = std::thread::spawn(move || -> Result<Vec<(PathBuf, u64, u64)>, Error> {
            
            let mut results = Vec::new();
            while let Some(task) = queue_clone.pop() {
                let (uniq, hl) = compute_dir_entry_size(&task, &global_hashmap_clone)?;
                results.push((task, uniq, hl)); 
            };
            
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

    println!("{:<12} | {:<12} | {:<10} | {:<10} | {:<10}", 
        "Host", "Snapshot", "Unique", "Hard-link", "Total"
    );
    for (snap, uniq_size, hl_size) in snapshot_sizes {
        
        let parent = snap.parent().and_then(|p| p.file_name().unwrap().to_str()).unwrap();
        let snapshot = snap.file_name().and_then(|p| p.to_str()).unwrap();

        println!("{:<12} | {:<12} | {:<10} | {:<10} | {:<10}", 
            parent,
            snapshot,
            format_size(uniq_size),
            format_size(hl_size),
            format_size(uniq_size + hl_size), 
        );
    } 

    Ok(())

}


/*
pub fn compute_snapshot_sizes() -> Result<(), Error> {
    let snapshot_dir = Path::new("/var/snaps/snapshots/arch-theo/");
    
    let mut snapshots: Vec<PathBuf> = snapshot_dir
        .read_dir()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
        })
        .map(|entry| entry.path())
        .collect();

    snapshots.sort();

    let (tx, rx) = std::sync::mpsc::channel();
    let thread_count = snapshots.len();

    for snap in snapshots {
        let tx_clone = tx.clone();
        std::thread::spawn(move || {
            let result = compute_dir_entry_size(&snap).map(|size| (snap, size));
            let _ = tx_clone.send(result);
        });
    }

    drop(tx);

    let mut result = Vec::with_capacity(thread_count);
    for received in rx {
        match received {
            Ok((snap, size)) => result.push((snap, size)),
            Err(e) => return Err(e),
        }
    }

    result.sort_by(|a, b| a.0.cmp(&b.0));

    println!("{:<12} | {:<12} | {:<10} | {:<10} | {:<10}", 
        "Host", "Snapshot", "Unique", "Hard-link", "Total"
    );
    for (snap, (uniq_size, hl_size)) in result {
        
        let parent = snap.parent().and_then(|p| p.file_name().unwrap().to_str()).unwrap();
        let snapshot = snap.file_name().and_then(|p| p.to_str()).unwrap();

        println!("{:<12} | {:<12} | {:<10} | {:<10} | {:<10}", 
            parent,
            snapshot,
            uniq_size,//human_uniq_size,
            hl_size,//human_hl_size,
            uniq_size + hl_size,//human_total_size 
        );
    }

    
    
    Ok(())
}
*/
