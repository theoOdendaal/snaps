use std::io::Write;

use crate::error::Error;

pub fn handle_remove(
    select_indexes: Option<Vec<usize>>,
    select_tags: Option<Vec<crate::meta::RetentionTag>>,
    force: bool,
) -> Result<(), Error> {
    let host_location = crate::location::get_host_location()?;
    let snapshots = crate::location::retrieve_snapshots(&host_location)?;

    let latest_location = crate::location::get_latest_location()?;
    let latest_location_target = std::fs::read_link(&latest_location)?;
    let latest_snapshot = latest_location_target.file_name().and_then(|s| s.to_str()?.parse::<u64>().ok());
    
    // Used to update the latest symlink,
    // if necessary.
    let mut latest_preceding = None;

    let metadata_file_content = crate::meta::read_metadata_file_to_string()?;
    let mut metadata = crate::meta::parse_metadata(&metadata_file_content)?;

    let metadata_count = metadata.len();

    let mut index_iter = select_indexes.into_iter().flatten().rev().peekable();

    // Snapshots are traversed from oldest to newest. This decision
    // has been made to more effeciently update 'latest'.
    // This does however require the symlink to not be the
    // oldest, as there won't be a latest preceding.
    for (i, snap) in snapshots.iter().enumerate().rev() {
        
        let tags = metadata[i].tags();

        let mut is_selected = select_tags
            .clone()
            .is_some_and(|selected| selected.iter().any(|t| tags.contains(t)));
        
        if !is_selected && let Some(idx) = index_iter.peek() {
            // FIXME: This logic requires that select_indexes be sorted
            // ascending.
                if *idx == i {
                    index_iter.next();
                    is_selected = true;
                } else if *idx > i {
                    index_iter.next();
                }
        }

        if !is_selected {
                latest_preceding = Some(snap);

        } else if is_selected {
            let is_latest = latest_snapshot.is_some_and(|l| l == *snap); 

            let snapshot_directory = crate::location::construct_snapshot_directory(*snap)?;

            let mut input = if force {
                String::from("y")
            } else {
                String::new()
            };
            while input.trim().to_lowercase() != "y" && input.trim().to_lowercase() != "n" {
                input.clear();
                print!("Confirm removal of {:?}, (y/n): ", snapshot_directory);
                std::io::stdout().flush()?;
                std::io::stdin().read_line(&mut input)?;
            }

            if input.trim().to_lowercase() == "y" {
                println!("Removing: {:?}", snapshot_directory);

                metadata.remove(i);


                if is_latest {
                    match latest_preceding {
                        Some(prec) => {
                            let link_directory = crate::location::construct_snapshot_directory(*prec)?;
                            crate::location::set_latest_symlink(&link_directory)?;
                        }
                        // This branch will be reached if the 'latest' symlink
                        // is the oldest current snapshot.
                        // FIXME: This should be improved, although the
                        // oldest snapshot being the 'latest' is unlikely,
                        // it will still cause problems.
                        None => { crate::location::remove_latest_symlink()?; }
                    } 
                }
                
                remove_snapshot(&snapshot_directory)?;
            }
        }
    }

    if metadata_count != metadata.len() {
        crate::meta::SnapshotMetaData::overwrite_metadata_file(&metadata)?;
    }

    Ok(())
}

fn remove_snapshot(snapshot_path: &std::path::Path) -> Result<(), Error> {
    let file_name = snapshot_path.file_name().unwrap().to_string_lossy();

    let parent = snapshot_path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid snapshot path")
    })?;

    let trash_dir = parent.join(".trash");
    std::fs::create_dir_all(&trash_dir)?;

    let trash_name = trash_dir.join(format!("tmp_{}", file_name));

    std::fs::rename(snapshot_path, &trash_name)?;

    std::fs::remove_dir_all(trash_name)?;

    Ok(())
}
