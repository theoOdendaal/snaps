// used to validate existing snapshots and configuration
// files.

use crate::error::Error;

pub const ANSI_WARNING: &str = "\x1B[38;5;3m";
pub const ANSI_INFO: &str = "\x1B[38;5;29m";
pub const ANSI_RESET: &str = "\x1B[0m";

// FIXME: Create functionality that will restore the 'latest'
// symlink if broken.

pub fn handle_checkhealth(dump: bool) -> Result<(), Error> {
    if dump {
        crate::meta::dump_snapshots()?;
    }

    let stdout = std::io::stdout();
    let mut handle = std::io::BufWriter::new(stdout.lock());

    let (_, host_path) = crate::location::get_host_dir_information()?;

    // Validate existence of 'latest' symlink.
    let latest_location = crate::location::get_latest_path(&host_path);
    if !latest_location.exists() || std::fs::symlink_metadata(latest_location).is_err() {
        print_broken_symlink(&mut handle)?;
    }

    // Reconcile the metadata file with the snapshot
    // directories.
    let metadata_file_content = crate::meta::read_metadata_file_to_string()?;
    let metadata = crate::meta::parse_metadata_as_ordered_vec(&metadata_file_content)?;

    let snapshots = crate::location::retrieve_snapshots_as_ordered_vec(&host_path)?;
    
    let mut m_iter = metadata.iter().map(|m| m.timestamp()).peekable();
    let mut s_iter = snapshots.iter().peekable();


    // Both metadata and snapshots are ordered from
    // greatest to smallest.
    while m_iter.peek().is_some() || s_iter.peek().is_some() {

        match (m_iter.peek(), s_iter.peek()) {
            (Some(a), Some(b)) => {
                
                if a == *b {
                    print_matched(&mut handle, a)?;
                    m_iter.next();
                    s_iter.next();
                } else if a < *b {
                    print_metadata_incomplete(&mut handle, b)?;
                    s_iter.next();
                } else {
                    print_missing_snapshot(&mut handle, a)?;
                    m_iter.next();
                } 

            },

            (Some(a), None) => {
                print_missing_snapshot(&mut handle, a)?;
                m_iter.next();
            },
            
            (None, Some(b)) => {
                print_metadata_incomplete(&mut handle, b)?;
                s_iter.next();
            },

            (None, None) => { unreachable!() },

        }
    }

    Ok(())
}

fn print_matched<W: std::io::Write>(writer: &mut W, timestamp: &u64) -> std::io::Result<()> {
    writeln!(writer, "{ANSI_INFO}{timestamp} -> Matched{ANSI_RESET}")
}

fn print_metadata_incomplete<W: std::io::Write>(writer: &mut W, timestamp: &u64) -> std::io::Result<()> {
    writeln!(writer, "{ANSI_WARNING}{timestamp} -> Metadata incomplete{ANSI_RESET}")
}

fn print_missing_snapshot<W: std::io::Write>(writer: &mut W, timestamp: &u64) -> std::io::Result<()> {
    writeln!(writer, "{ANSI_WARNING}{timestamp} -> Missing snapshot{ANSI_RESET}")
}

fn print_broken_symlink<W: std::io::Write>(writer: &mut W) -> std::io::Result<()>{
    writeln!(writer, "{ANSI_WARNING}Broken 'latest' symlink{ANSI_RESET}")
}

