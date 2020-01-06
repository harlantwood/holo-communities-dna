use std::time::{Duration};
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

#[derive(Serialize, Deserialize, Debug, Clone, DefaultJson, PartialEq)]
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
        let mut sorted_divisions = divisions.clone();
        sorted_divisions.sort();
        Self {
            smallest_time_block,
            divisions: sorted_divisions,
        }
    }

    /**
     * @brief      Given a timestamp returns the path in the anchor tree where
     *             something with this timestamp can be found/should be placed.
     *             The paths go from the leaves up and the root is not included.
     *
     * @return     A path definition as a vector of time anchors
     */
    pub fn entry_path_from_timestamp(&self, timestamp_ms: u128) -> Vec<TimeAnchor> {
        vec![1].iter().chain(self.divisions.iter()).enumerate().map(|(height, blocks)| {
            let block_number = timestamp_ms / ((*blocks as u128) * self.smallest_time_block.as_millis());
            TimeAnchor(format!("{}-{}", height, block_number))  // Uses encoding where "2-100" would be be the 3rd layer from the bottom, 100th anchor
                                                                // This is not super important but ensures that each anchor string is unique
        }).collect()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn can_create_path_for_timestamp_first_block() {
        // anchors for every minute, 5 minutes and 10 minutes
        let spec = TimeAnchorTreeSpec::new(Duration::from_secs(60), vec![5, 10]);
        let path = spec.entry_path_from_timestamp(100); // strange timestamp 100ms after the epoch :P
        assert_eq!(
            path,
            vec![
                TimeAnchor("0-0".to_string()),
                TimeAnchor("1-0".to_string()),
                TimeAnchor("2-0".to_string())
            ]
        );
    }

    #[test]
    fn can_create_path_for_timestamp_later_blocks() {
        // anchors for every minute, 5 minutes and 10 minutes
        let spec = TimeAnchorTreeSpec::new(Duration::from_secs(60), vec![5, 10]);
        let path = spec.entry_path_from_timestamp(1000*60*11); // 11 minutes after the epoch
        assert_eq!(
            path,
            vec![
                TimeAnchor("0-11".to_string()),
                TimeAnchor("1-2".to_string()),
                TimeAnchor("2-1".to_string())
            ]
        );
    }
}
