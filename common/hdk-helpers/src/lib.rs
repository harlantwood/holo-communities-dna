use std::time::{Duration, SystemTime};
use hdk::prelude::*;

pub fn commit_if_not_in_chain(entry: &Entry) -> ZomeApiResult<Address> {
    // use query to check the chain. When there is an HDK function doing this directly use it instead
    let existing_entries = hdk::query(entry.entry_type().into(), 0, 0)?;
    if existing_entries.contains(&entry.address()) {
        // do nothing and be happy
        Ok(entry.address())
    } else {
        // do the commit as usual
        hdk::commit_entry(entry)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, DefaultJson)]
pub struct TimeAnchor(String);

pub struct TimeAnchorTreeSpec {
    smallest_time_block: Duration, // size of smallest unit (e.g. 1 hour)
    divisions: Vec<u32>, // how the tree will be structured in terms of the smallest units
                         // e.g. a tree structured hours -> days -> months would be:
                         // smallest_time_block = Duration::from_secs(60*60) 
                         // divisions = vec![24, 24*30]
}

impl TimeAnchorTreeSpec {
    pub fn new(smallest_time_block: Duration, divisions: Vec<u32>) -> Self {
        let sorted_divisions = divisions.clone();
        sorted_divisions.sort();
        Self {
            smallest_time_block,
            divisions: sorted_divisions,
        }
    }

    /**
     * @brief      Given a timestamp returns the path in the anchor tree where
     *             something with this timestamp can be found/should be placed
     *
     * @return     A path definition as a vector of time anchors
     */
    pub fn entry_path_from_timestamp(&self, timestamp: SystemTime) -> Vec<TimeAnchor> {
        
    }
}
