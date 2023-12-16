# gossip

A tiny `gossip` crate based on UDP messaging for node membership.

Concept:

1. A node starts up, it may be provided with an initial list of servers that are known, known as its "seed peers".
2. The node broadcasts its existence within its network.
3. Nodes which receive this broadcast will add the new node to their peers and send back their known peers.


This implementation is only used by the `dnd` application for simplistic replication.
