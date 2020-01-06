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
    let base_entry = Entry::App(TIME_ANCHOR_ENTRY_TYPE.into(), TimeAnchor(base.clone()).into());
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
    let path = time_anchor_spec().entry_path_from_timestamp(timestamp);
    // commit every anchor on the path (that doesn't already exist)
    let mut last_anchor_addr = base_address;
    for anchor in path.iter().rev() { // need to reverse so the links can be created in order
        let anchor_entry = Entry::App(
            TIME_ANCHOR_ENTRY_TYPE.into(),
            anchor.into()
        );
        if !hdk_helpers::is_in_chain(&anchor_entry)? && !hdk_helpers::is_in_dht(&anchor_entry)? {
            hdk::commit_entry(&anchor_entry)?;
        }
        // link the chain of anchors
        hdk::link_entries(&last_anchor_addr, &anchor_entry.address(), TIME_ANCHOR_TO_TIME_ANCHOR_LINK_TYPE, "")?;
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
 *                         
 * @param      limit       Number of posts to return as a maximum. If this limit is hit will return true for the more boolean
 *
 * @return     Returns a tuple of the returned entries/addresses and a bool which is true if there are more posts available
 */
pub fn all_for_base(
    base: String,
    limit: Option<usize>,
) -> ZomeApiResult<GetPostsResult> {

    // start at the root and traverse the tree taking the newest branch each time
    // repeat until `limit` leaves/posts have been visited
    let base_address = Entry::App(TIME_ANCHOR_ENTRY_TYPE.into(), TimeAnchor(base).into()).address();
    let mut to_visit = vec![base_address];
    let mut post_addrs = Vec::new();
    let mut more = false;
    while let Some(current) = to_visit.pop() {
        // add the children to the stack
        for addr in hdk::get_links(&current, LinkMatch::Exactly(TIME_ANCHOR_TO_TIME_ANCHOR_LINK_TYPE), LinkMatch::Any)?.addresses() {
            to_visit.push(addr)
        }
        // add any post children to the result (should only be on leaves)
        for post_addr in hdk::get_links(&current, LinkMatch::Exactly(TIME_ANCHOR_TO_POST_LINK_TYPE), LinkMatch::Any)?.addresses() {
            if let Some(limit) = limit {
                if post_addrs.len() < limit {
                    post_addrs.push(post_addr);
                // } else {
                    more = true;
                    break;
                }
            } else {
                post_addrs.push(post_addr);              
            }
        }
    }

    // actually load all the posts from their addresses
    let posts = post_addrs.into_iter().map(|addr| {
        let post: Post = hdk::utils::get_as_type(addr.clone()).unwrap();
        post.with_address(addr)
    }).collect();

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