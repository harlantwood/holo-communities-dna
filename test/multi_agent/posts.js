const { one } = require('../config')

module.exports = (scenario) => {

const postFactory = (title) => ({
    title,
    details: "this is a details string",
    post_type: "a type",
    announcement: false,
    timestamp: "",
    base: "community1",
})

scenario('Can create multiple posts and paginate with two agents', async (s, t) => {
    const { alice, bob } = await s.players({alice: one, bob: one}, true)

    const nTestPosts = 10
    let postAddrs = []
    for(let i = 0; i < nTestPosts; i++) {
      const testPost = postFactory("test"+i)
      // switch agents every second post
      const add_post_result = i % 2 == 0 ? await alice.callSync("app", "posts", "create", testPost ) : await bob.callSync("app", "posts", "create", testPost )
      await s.consistency()
      t.equal(add_post_result.Ok.address.length, 46)
      postAddrs.push(add_post_result.Ok.address)
    }

    await s.consistency()

    // print the adjacancy list from both agents perspective
    const adj_alice = await alice.call("app", "posts", "adjacency_list_for_base", {base: postFactory("").base} )
    const adj_bob = await bob.call("app", "posts", "adjacency_list_for_base", {base: postFactory("").base} )
    t.ok(adj_alice.Ok, "alice could get adjacency list")
    t.ok(adj_bob.Ok, "bob could get adjacency list")
    t.deepEqual(adj_alice.Ok, adj_bob.Ok, "Alice and Bob see the same graph structure")
    
    // optinally prettyify it for mermaid.js displaying
    let edges = adj_alice.Ok
    for (let i=0; i<edges.length; i++) {
      let edge = edges[i];
      console.log(`${postAddrs.indexOf(edge[0])} --> ${postAddrs.indexOf(edge[1])}`)
    }

    let agents = [alice, bob];
    // where is my for..in loop javascript!
    for(let i=0; i<agents.length; i++) {
      let agent = agents[i]
      // try getting all of them
      const get_posts_result = await agent.callSync("app", "posts", "all_for_base", {
        base: postFactory("").base,
      })
      t.deepEqual(get_posts_result.Ok.posts.length, nTestPosts, "Could not get all posts for agent"+i)

      // try getting only the first slicePoint
      const slicePoint = 7;
      const get_posts_result_limit = await agent.callSync("app", "posts", "all_for_base", {
        base: postFactory("").base,
        limit: slicePoint
      })
      t.deepEqual(get_posts_result_limit.Ok.more, true)  
      t.deepEqual(get_posts_result_limit.Ok.posts.map(p => p.address), postAddrs.slice(0, slicePoint), "Incorrect limited post retrieval for agent"+i)  

      // try getting the rest
      const get_posts_result_since = await agent.callSync("app", "posts", "all_for_base", {
        base: postFactory("").base,
        since: postAddrs[slicePoint-1]
      })
      t.deepEqual(get_posts_result_since.Ok.more, false)  
      t.deepEqual(get_posts_result_since.Ok.posts.map(p => p.address), postAddrs.slice(slicePoint, nTestPosts), "incorrect since post retrieval for agent"+i)  
    }
  })

}
