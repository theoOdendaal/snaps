use std::{collections::VecDeque, path::{Path, PathBuf}, time::SystemTime};

use crate::error::Error;
use crate::filter::should_ignore;

pub fn handle_take() -> Result<(), Error>{
    let snapshot_dir = create_snapshot()?;

    let start = Path::new("/");

    orchestrate_parallel_fs_walk(start, &snapshot_dir)?;

    set_latest_symlink(&snapshot_dir)?;

    Ok(())
}

fn create_snapshot() -> Result<PathBuf, Error> {
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();

    let location = crate::location::get_host_location()?
        .join(timestamp.to_string());

    if location.exists() {
        return Err(Error::Io(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "Snapshot directory already exists",
                )));
    }

    std::fs::create_dir_all(&location)?;
    assert!(location.is_absolute());

    Ok(location)
}

// Update 'latest' symlink to reference a new snapshot,
// after removing the existing symlink.
fn set_latest_symlink(snapshot_dir: &Path) -> Result<(), Error> {
    let link = crate::location::get_latest_location()?;

    if link.exists() || std::fs::symlink_metadata(&link).is_ok() {
        std::fs::remove_file(&link)?;
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(snapshot_dir, &link)?;

    Ok(())
}

// Determines whether source is identical to target
// based on size and last modification time.
fn check_file_change(source: &Path, target: &Path) -> Result<bool, Error> {
    let target_metadata = std::fs::metadata(target)?;
    let source_metadata = std::fs::metadata(source)?;

    let target_modified = target_metadata.modified()?;
    let source_modified = source_metadata.modified()?;

    Ok((target_metadata.len() != source_metadata.len()) || (target_modified != source_modified))
}

fn incremental_copy(source_dir: &Path, target_dir: &Path) -> Result<(), Error> {
    assert!(source_dir != target_dir);
    
    let relative_source_dir = source_dir
        .strip_prefix(std::path::Component::RootDir)
        .map_err(std::io::Error::other)?; 
    
    let latest_location = crate::location::get_latest_location()?;
    let previous_snapshot = latest_location.join(relative_source_dir);
    
    // Safety net if CreateDir task is not complete.
    if let Some(parent) = target_dir.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if !previous_snapshot.exists() || check_file_change(source_dir, &previous_snapshot)? {

        std::fs::copy(source_dir, target_dir)?;

        let metadata =  std::fs::metadata(source_dir)?;
        let permissions = metadata.permissions();
        std::fs::set_permissions(target_dir, permissions)?;

        if let (Ok(accessed), Ok(modified)) = (metadata.accessed(), metadata.modified()) {
            let times = std::fs::FileTimes::new()
                .set_accessed(accessed)
                .set_modified(modified);
            
            let target_buffer = std::fs::File::open(target_dir)?;
            target_buffer.set_times(times)?;
            
            //FIXME: std::fs::set_times is not yet stable.
            //For now, I'll have to open a buffer.
            //std::fs::set_times(target_dir, times)?;
            
            println!("{:?} -> {:?}",source_dir, target_dir);
        }

    } else  {
        std::fs::hard_link(&previous_snapshot, target_dir)?;

    }

    Ok(())
}

enum Task {
    CreateDir { target_dir: PathBuf },
    CopyDir { source_dir: PathBuf, target_dir: PathBuf },
}

struct TaskQueue {
    queue: std::sync::Mutex<VecDeque<Task>>,
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

    fn push(&self, task: Task) {
        let mut q = self.queue.lock().unwrap();
        q.push_back(task);
        self.condvar.notify_one();
    }

    fn set_finished(&self) {
        let mut finished = self.finished.lock().unwrap();
        *finished = true;
        self.condvar.notify_all();
    }

    fn pop(&self) -> Option<Task> {
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

fn orchestrate_parallel_fs_walk(path: &Path, snapshot_dir: &Path) -> Result<(), Error> {

    let task_queue= std::sync::Arc::new(TaskQueue::new());

    let num_workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    
    let mut handles = vec![];

    for _ in 0..num_workers {
        let queue_clone = std::sync::Arc::clone(&task_queue);

        let handle = std::thread::spawn(move || -> Result<(), Error> {

            while let Some(task) = queue_clone.pop() {
                match task {
                    Task::CreateDir { target_dir } => { std::fs::create_dir_all(&target_dir)? },
                    Task::CopyDir { source_dir, target_dir } =>  { incremental_copy(&source_dir, &target_dir)? },
                }
            }

            Ok(())
        });
        handles.push(handle);
    }

    walk_fs_parallel_iter(path, snapshot_dir, &task_queue)?;
    task_queue.set_finished();

    for handle in handles {
        match handle.join() {
            Ok(thread_results) => thread_results?,
            Err(e) => std::panic::resume_unwind(e),
        }
    } 

    Ok(())
}

fn walk_fs_parallel_iter(path: &Path, snapshot_dir: &Path, task_queue: &TaskQueue) -> Result<(), Error> {
    
    assert!(path.is_absolute());
    assert!(snapshot_dir.is_absolute());

    let mut stack = vec![path.to_path_buf()];

    while let Some(path) = stack.pop() {
        let entries = match std::fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(_) => continue
        };

        for entry in entries.flatten() {
            let path = entry.path();

            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            
            // Symlinks are always ignored.
            // Or at least I replicate them
            // as part of the snapshot.
            if file_type.is_symlink() {
                continue;

            } else if file_type.is_dir() && !should_ignore(&path) {
                stack.push(path.clone()); 

                let relative_path = path
                    .strip_prefix(std::path::Component::RootDir)
                    .map_err(std::io::Error::other)?;

                let target_dir = snapshot_dir.join(relative_path);
                
                task_queue.push(Task::CreateDir { target_dir });

            } else if file_type.is_file() {
                let relative_path = path
                    .strip_prefix(std::path::Component::RootDir)
                    .map_err(std::io::Error::other)?;

                // If the value passed to Path::join is absolute
                // the output is only the value.
                assert!(snapshot_dir.is_absolute());
                assert!(relative_path.is_relative());
                
                let target_path = snapshot_dir.join(relative_path);
                
                task_queue.push(Task::CopyDir { source_dir: path, target_dir: target_path});
            }
        }
    }
    Ok(())
}
