# dnd (definitely not dynamo)

A distributed key-value store, inspired by [Amazon's Dynamo](https://www.amazon.science/publications/dynamo-amazons-highly-available-key-value-store) implementation.

After reading their paper, I wanted to implement something similar so I whipped up a stripped down version with my own quirks - `dnd` is the result of that.

## Functionality

As `dnd` is inspired by Dynamo, it matches some of their initial criteria:

- Symmetry: There are no specific _roles_ for nodes, i.e. there is no concept of a leader node with followers. Instead, all nodes communicate peer-to-peer via a gossip protocol.
- Replication: The current implementation does not use consistent hashing, the trade-off here is that the entire K-V store is replicated to all nodes, based on the latest `AtomicU64` counter. This would have major downsides in a huge cluster of nodes; however, it does mean that any node can respond to a `get(key)`.
- Availability: Writes are never rejected, the "last write wins".

**TODO: fill in during impl**


