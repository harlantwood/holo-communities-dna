use hdk::{
    self,
    entry_definition::ValidatingEntryType,
    error::ZomeApiResult,
    holochain_core_types::{dna::entry_types::Sharing, entry::Entry},
    holochain_json_api::{
        error::JsonError,
        json::{JsonString, RawString},
    },
    holochain_persistence_api::cas::content::{Address},
    utils, AGENT_ADDRESS,
};

#[derive(Serialize, Deserialize, Debug, Clone, DefaultJson)]
pub struct Post {
    pub title: String,
    pub details: String,
    pub post_type: String,
    pub creator: Address,
    pub announcement: bool,
    pub timestamp: String,
    pub base: String,
    // fields for the dag list
    prev_authored: Address,
    prev_foreign: Address,
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
    pub timestamp: String,
    pub base: String,
}

pub type Base = RawString;

const POST_ENTRY_TYPE: &str = "post";
const POST_BASE_ENTRY: &str = "post_base";
const POST_LINK_TYPE: &str = "posted_in";

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
    timestamp: String,
) -> ZomeApiResult<PostWithAddress> {
    let base_entry = Entry::App(POST_BASE_ENTRY.into(), RawString::from(base.clone()).into());
    let _base_address = hdk::commit_entry(&base_entry)?;

    let post: Post = Post {
        title,
        details,
        post_type,
        creator: AGENT_ADDRESS.to_string().into(),
        announcement,
        timestamp,
        base: base.clone(),
        prev_authored: Address::new(), // these will get overwritten
        prev_foreign: Address::new(),
    };

    let post_address = hdk::commit_entry(&Entry::App(POST_ENTRY_TYPE.into(), post.clone().into()))?;

    Ok(post.with_address(post_address))
}

/**
 * @brief      Traverse the graph and recover all the posts (possibly up to a given limit)
 *
 * @param      base        The base/community for these posts. This is a string and can be considered equivalent to a database table name
 *
 * @param      since       The starting point for the traversal. Can be the address of a community, or another post.
 *                         If it is a post it will only return those occurring later (allowing for pagination)
 *                         
 * @param      limit       Number of posts to return as a maximum. If this limit is hit will return true for the more boolean
 *
 * @param      backsteps   Number of backward steps to take in the graph before beginning the traversal.
 *                         This is because it cannot be guaranteed that all posts will be retrieved with a forward only traversal.
 *                         
 *
 * @return     Returns a tuple of the returned entries/addresses and a bool which is true if there are more posts available
 */
pub fn all_for_base(
    base: String,
    timestamp: u64,
    since: Option<Address>,
    limit: Option<usize>,
) -> ZomeApiResult<GetPostsResult> {
    Ok(GetPostsResult { posts: Vec::new(), more: false })
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
        links: [
            to!(
                POST_ENTRY_TYPE,
                link_type: "dag/next",
                validation_package: || {
                    hdk::ValidationPackageDefinition::Entry
                },
                validation: | _validation_data: hdk::LinkValidationData| {
                    Ok(())
                }
            ),
            from!(
                "%agent_id",
                link_type: "dag/author_root",
                validation_package: || {
                    hdk::ValidationPackageDefinition::Entry
                },
                validation: | _validation_data: hdk::LinkValidationData| {
                    Ok(())
                }
            ),
            from!(
                POST_BASE_ENTRY,
                link_type: "dag/foreign_root",
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

pub fn base_def() -> ValidatingEntryType {
    entry!(
        name: POST_BASE_ENTRY,
        description: "Universally unique ID of something that is being posted in",
        sharing: Sharing::Public,
        validation_package: || {
            hdk::ValidationPackageDefinition::Entry
        },
        validation: | _validation_data: hdk::EntryValidationData<Base>| {
            Ok(())
        },
        links: [
            to!(
                POST_ENTRY_TYPE,
                link_type: POST_LINK_TYPE,
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
