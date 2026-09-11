// used to validate existing snapshots and configuration
// files.

use crate::error::Error;

pub const ANSI_WARNING: &str = "\x1B[38;5;3m";
pub const ANSI_INFO: &str = "\x1B[38;5;29m";
pub const ANSI_RESET: &str = "\x1B[0m";

pub fn handle_checkhealth(dump: bool) -> Result<(), Error> {
    
    if dump {
        crate::meta::dump_snapshots()?;
        return Ok(());
    }

    // Validate existence of 'latest' symlink.
    let latest_location = crate::location::get_latest_location()?;
    if !latest_location.exists() || std::fs::symlink_metadata(latest_location).is_err() {
        print_broken_symlink();
    }

    // Reconcile the metadata file with the snapshot
    // directories.
    let metadata_file_content = crate::meta::read_metadata_file_to_string()?;
    let metadata = crate::meta::parse_metadata(&metadata_file_content)?;

    let host_location = crate::location::get_host_location()?;
    let snapshots = crate::location::retrieve_snapshots(&host_location)?;
    
    let mut m_iter = metadata.iter().map(|m| m.timestamp()).peekable();
    let mut s_iter = snapshots.iter().peekable();

    // Both metadata and snapshots are ordered from
    // greatest to smallest.
    while m_iter.peek().is_some() || s_iter.peek().is_some() {

        match (m_iter.peek(), s_iter.peek()) {
            (Some(a), Some(b)) => {
                
                if a == *b {
                    print_matched(a);
                    m_iter.next();
                    s_iter.next();
                } else if a < *b {
                    print_metadata_incomplete(b);
                    s_iter.next();
                } else {
                    print_missing_snapshot(a);
                    m_iter.next();
                } 

            },

            (Some(a), None) => {
                print_missing_snapshot(a);
                m_iter.next();
            },
            
            (None, Some(b)) => {
                print_metadata_incomplete(b);
                s_iter.next();
            },

            (None, None) => { unreachable!() },

        }
    }

    Ok(())
}

fn print_matched(timestamp: &u64) {
    println!("{ANSI_INFO}{timestamp} -> Matched{ANSI_RESET}")
}

fn print_metadata_incomplete(timestamp: &u64) {
    println!("{ANSI_WARNING}{timestamp} -> Metadata incomplete{ANSI_RESET}");
}

fn print_missing_snapshot(timestamp: &u64) {
    println!("{ANSI_WARNING}{timestamp} -> Missing snapshot{ANSI_RESET}");
}

fn print_broken_symlink() {
    println!("{ANSI_WARNING}Broken 'latest' symlink{ANSI_RESET}");
}

