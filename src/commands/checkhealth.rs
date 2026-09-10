
// checkhealth should never fail, as it should be
// used to validate existing snapshots and configuration
// files.

use crate::error::Error;

pub fn handle_checkhealth() -> Result<(), Error> {
    
    let metadata_file_content = crate::meta::read_metadata_file_to_string()?;
    let metadata = crate::meta::parse_metadata(&metadata_file_content)?;
    let metadata_count = metadata.len();


    let host_location = crate::location::get_host_location()?;
    let snapshots = crate::location::retrieve_snapshots(&host_location)?;
    let snapshot_count = snapshots.len();

    if metadata_count != snapshot_count {
        println!("Disconnect between metadata count {} and snapshot count {}", metadata_count, snapshot_count);
    }

    

 
    Ok(()) 


}


