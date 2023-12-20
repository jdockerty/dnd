# dnd (definitely not dynamo)

A toy implementation of a distributed key-value store, inspired by [Amazon's Dynamo](https://www.amazon.science/publications/dynamo-amazons-highly-available-key-value-store).

After reading their paper, I wanted to implement something similar so I whipped up a stripped down version with my own quirks - `dnd` is the result of that.

## Functionality

As `dnd` is inspired by Dynamo, it matches some of their initial criteria:

- Symmetry: There are no specific _roles_ for nodes, i.e. there is no concept of a leader node with followers. Instead, all nodes communicate peer-to-peer via a gossip protocol.
- Replication: The current implementation does not use consistent hashing, the trade-off here is that the entire K-V store is replicated to all nodes, based on the latest `AtomicU64` counter. This would have major downsides in a huge cluster of nodes; however, it does mean that any node can respond to a `get(key)`.
- Availability: Writes are never rejected, the "last write wins".


_TODO:_

- Failure detection, currently a node is not considered unhealthy if it goes away, UDP packets are simply sent into the void.
- Consistent hashing (mentioned above)


## How it works

The `gossip` implementation and a HTTP server run on separate threads. The HTTP server is used to accept `get` and `put` operations
via HTTP requests. For example

```bash
curl -X POST localhost:6000/kv/user1 -d '{"name": "Jack"}'
```

The above will _put_ a key called `user1` with the JSON payload provided into the store, provided via [`dashmap`](https://docs.rs/dashmap/latest/dashmap/).

Whilst the HTTP server waits for requests to cover `get(key)` and `put(key, data)` operations, the distributed key-value store functionality is handled by my stripped down implementation of [SWIM](https://www.cs.cornell.edu/projects/Quicksilver/public_pdfs/SWIM.pdf), a gossip protocol.
A node can either `start` a new cluster or `join` an existing one by specifying a known `Peer`. At every interval, either a `read` or `write` operation will occur:

- Write means that a random peer node will be selected to send an `Update` to, containing the store and its current atomic counter.
- Read means that a UDP datagram will be read from the bound socket and unmarshaled, this does not always contain new information in which case nothing happens.

The most up to date `DashMap` will always win, this is tracked by an [`AtomicU64`](https://doc.rust-lang.org/std/sync/atomic/index.html) counter. When a put operation occurs, the counter is incremented. This means that when a read
sees that the incoming data is more up to date than its own, its `DashMap` will be replaced by the one contained in the `Update`.
