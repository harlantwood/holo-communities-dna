use hdk::{
    self,
    entry_definition::ValidatingEntryType,
    error::ZomeApiResult,
    holochain_core_types::{dna::entry_types::Sharing, entry::Entry},
    holochain_json_api::{
        error::JsonError,
        json::{JsonString},
    },
    holochain_persistence_api::cas::content::{Address, AddressableContent},
    utils, AGENT_ADDRESS,
};
use hdk::prelude::*;
use hdk_helpers::{TimeAnchorTreeSpec, TimeAnchor};
use std::time::Duration;
use std::convert::TryFrom;
use std::collections::BinaryHeap;
// use itertools::Itertools;

// This specifices the stucture of the time anchors.
// This has the structure hours->days->weeks
fn time_anchor_spec() -> TimeAnchorTreeSpec {
    TimeAnchorTreeSpec::new(
        Duration::from_secs(60*60),
        vec![24, 24*7], 
    )
}


#[derive(Serialize, Deserialize, Debug, Clone, DefaultJson)]
pub struct Post {
    pub title: String,
    pub details: String,
    pub post_type: String,
    pub creator: Address,
    pub announcement: bool,
    pub timestamp: u128,
    pub base: String,
}

impl Post {
    pub fn with_address(&self, address: Address) -> PostWithAddress {
        PostWithAddress {
            address,
            title: self.title.clone(),
            details: self.details.clone(),
            post_type: self.post_type.clone(),
            creator: self.creator.clone(),
            announcement: self.announcement.clone(),
            timestamp: self.timestamp.clone(),
            base: self.base.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, DefaultJson)]
pub struct PostWithAddress {
    pub address: Address,
    pub title: String,
    pub details: String,
    pub post_type: String,
    pub creator: Address,
    pub announcement: bool,
    pub timestamp: u128,
    pub base: String,
}

const POST_ENTRY_TYPE: &str = "post";
const TIME_ANCHOR_ENTRY_TYPE: &str = "post_time_anchor";
const TIME_ANCHOR_TO_TIME_ANCHOR_LINK_TYPE: &str = "time_anchor_to_time_anchor";
const TIME_ANCHOR_TO_POST_LINK_TYPE: &str = "time_anchor_to_post";

#[derive(Serialize, Deserialize, Debug, Clone, DefaultJson)]
pub struct GetPostsResult {
    posts: Vec<PostWithAddress>,
    more: bool,
}

pub fn get(address: Address) -> ZomeApiResult<PostWithAddress> {
    utils::get_as_type::<Post>(address.clone()).map(|post| post.with_address(address))
}

pub fn create(
    base: String,
    title: String,
    details: String,
    post_type: String,
    announcement: bool,
    timestamp: u128,
) -> ZomeApiResult<PostWithAddress> {
    let base_entry = Entry::App(TIME_ANCHOR_ENTRY_TYPE.into(), TimeAnchor::new(0, 0, base.clone()).into());
    let base_address = hdk::commit_entry(&base_entry)?;

    let post: Post = Post {
        title,
        details,
        post_type,
        creator: AGENT_ADDRESS.to_string().into(),
        announcement,
        timestamp: timestamp,
        base: base.clone(),
    };

    let post_address = hdk::commit_entry(&Entry::App(POST_ENTRY_TYPE.into(), post.clone().into()))?;

    // get the anchor path required for this timestamp
    let path = time_anchor_spec().entry_path_from_timestamp(timestamp, &base);
    // commit every anchor on the path (that doesn't already exist)
    let mut last_anchor_addr = base_address;
    for anchor in path.iter().rev() { // need to reverse so the links can be created in order
        let anchor_entry = Entry::App(
            TIME_ANCHOR_ENTRY_TYPE.into(),
            anchor.into()
        );
        if !hdk_helpers::is_in_chain(&anchor_entry)? && !hdk_helpers::is_in_dht(&anchor_entry)? {
            hdk::commit_entry(&anchor_entry)?;
            // link the chain of anchors
            // OOoh tricky - put the entry in the tag of the link to save loads of get_entry calls later on ;)
            hdk::link_entries(&last_anchor_addr, &anchor_entry.address(), TIME_ANCHOR_TO_TIME_ANCHOR_LINK_TYPE, &JsonString::from(anchor).to_string())?;
        }
        last_anchor_addr = anchor_entry.address();
    }
    
    // finally hook the post on to the end
    hdk::link_entries(&last_anchor_addr, &post_address, TIME_ANCHOR_TO_POST_LINK_TYPE, "")?;

    Ok(post.with_address(post_address))
}

/**
 * @brief      Traverse the graph and recover all the posts (possibly up to a given limit)
 *
 * @param      base        The base/community for these posts. This is a string and can be considered equivalent to a database table name
 * @param      limit       Number of posts to return as a maximum. If this limit is hit will return true for the more boolean    
 * @param      before      Only return posts that occur before this timestamp (ms since unix epoch)
 * @return     Returns a tuple of the returned entries/addresses and a bool which is true if there are more posts available
 */
pub fn all_for_base(
    base: String,
    limit: Option<usize>,
    before: Option<u128>,
) -> ZomeApiResult<GetPostsResult> {

    // start at the root and traverse the tree taking the newest branch each time
    // repeat until `limit` leaves/posts have been visited
    let base_anchor = TimeAnchor::new(0, 0, base);
    let mut to_visit = BinaryHeap::new();
    to_visit.push(base_anchor);
    let mut posts = Vec::new();
    let mut more = false;
    while let Some(current) = to_visit.pop() {
        // add the children to the stack such that newest are visited first
        let current_addr = Entry::App(TIME_ANCHOR_ENTRY_TYPE.into(), current.into()).address();
        for link in hdk::get_links(&current_addr, LinkMatch::Exactly(TIME_ANCHOR_TO_TIME_ANCHOR_LINK_TYPE), LinkMatch::Any)?.links() {
            // only add links to visit if they are before the `before` timestamp (if it is given)
            let time_anchor = TimeAnchor::try_from(JsonString::from_json(&link.tag)).unwrap();
            if let Some(before) = before {
                if time_anchor.start_timestamp < before {
                    to_visit.push(time_anchor)
                }
            } else {
                to_visit.push(time_anchor)
            }
            
        }
        // add any post children to the result (should only be on leaves) (also in descending timestamp order)
        let mut posts_on_base: Vec<Post> = utils::get_links_and_load_type(&current_addr, LinkMatch::Exactly(TIME_ANCHOR_TO_POST_LINK_TYPE), LinkMatch::Any)?;
        posts_on_base.sort_by(|a, b| {
            b.timestamp.cmp(&a.timestamp)
        });
        for post in posts_on_base {
            let post_address = Entry::App(POST_ENTRY_TYPE.into(), post.clone().into()).address();
            match (limit, before) {
                (Some(limit), Some(before)) => {
                    if posts.len() < limit {
                        if post.timestamp < before {
                            posts.push(post.with_address(post_address));
                        }
                    } else {
                        more = true;
                        break;
                    }                    
                },
                (Some(limit), _) => {
                    if posts.len() < limit {
                        posts.push(post.with_address(post_address));
                    } else {
                        more = true;
                        break;
                    }
                },
                (_, Some(before)) => {
                    if post.timestamp < before {
                        posts.push(post.with_address(post_address));
                    }
                },
                (_, _) => {
                    posts.push(post.with_address(post_address))
                }
            }
        }
    }
    Ok(GetPostsResult { posts, more })
}

pub fn post_def() -> ValidatingEntryType {
    entry!(
        name: POST_ENTRY_TYPE,
        description: "",
        sharing: Sharing::Public,
        validation_package: || {
            hdk::ValidationPackageDefinition::Entry
        },
        validation: |_validation_data: hdk::EntryValidationData<Post>| {
            Ok(())
        },
        links: []
    )
}

pub fn post_time_anchor_def() -> ValidatingEntryType {
    entry!(
        name: TIME_ANCHOR_ENTRY_TYPE,
        description: "A timespan anchor",
        sharing: Sharing::Public,
        validation_package: || {
            hdk::ValidationPackageDefinition::Entry
        },
        validation: | _validation_data: hdk::EntryValidationData<TimeAnchor>| {
            Ok(())
        },
        links: [
            to!(
                TIME_ANCHOR_ENTRY_TYPE,
                link_type: TIME_ANCHOR_TO_TIME_ANCHOR_LINK_TYPE,
                validation_package: || {
                    hdk::ValidationPackageDefinition::Entry
                },
                validation: | _validation_data: hdk::LinkValidationData| {
                    Ok(())
                }
            ),
            to!(
                POST_ENTRY_TYPE,
                link_type: TIME_ANCHOR_TO_POST_LINK_TYPE,
                validation_package: || {
                    hdk::ValidationPackageDefinition::Entry
                },
                validation: | _validation_data: hdk::LinkValidationData| {
                    Ok(())
                }
            )
        ]
    )
}