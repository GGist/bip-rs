# Bittorrent Maineline DHT (bip_dht)
Implementation of the bittorrent mainline dht.

## Terminology

**Lookup**: Refers to the process of iteratively querying peers in the DHT to see if they have contact information for other peers
that have announced themselves for a given info hash.

**Announce**: Refers to the process of querying peers in the DHT, and telling the closest few nodes that you are interested in peers
looking for a particular info hash, and that the node should store your contact information for later nodes that reach any of the nodes
announced to.

## Important Usage Information
- **Before The Bootstrap**: It is always a good idea to start up the DHT ahead of time if you know you will need it later in your
application. This is because the DHT will not immediately be usable by us until bootstrapping has completed, you can feel free to
make requests, but they will be executed after the bootstrap has finished (which may take up to 30 seconds).

- **Announce Expire**: Nodes in the DHT will expire the contact information for announces that have gone stale (havent heard from again
for a while). This means if you are still looking for peers, you will want to announce periodically. All nodes have different expire
times, the spec mentions the 24 hour expire period, however, you may want to announce more often than that as peers are constantly leaving
and joining the DHT, so if the nodes you announced to all left the DHT, you would be out of luck. Luckily, for each announce, we do
replicate your contact information to multiple of the closest nodes.

- **Read Only Nodes**: By default, all nodes created are read only; this means that the node will not respond to requests. In theory
this sounds good, however, in practice this means it will be harder (but possible) to keep a healthy routing table, especially for
nodes that wish to run for long periods of time. I strongly encourage users who will be running nodes for long periods of time to
set up some sort of nat traversal/port forwarding to the source address of the DHT and set read only to false (it is true by default).

- **Source Port vs Connect Port**: One thing you should note is that, by either implementation error or intentionally, if the port that
the DHT is bound to is different than the port that we want nodes to connect to us on (our announce/connect port) some nodes will
incorrectly store the source port that we used to send the announce message instead of the port specified in the message. This is not
a big deal as most nodes handle this correctly ( I have only seen a few that screw this up). If you are receiving TCP connections requests
on the wrong port (the DHT source port), this is most likely why.

- **DHT Spam**: Many nodes in the DHT will ban nodes that they feel are malicious. This includes sending a high number
of requests, most likely for the same info hash, to the same node. As a user, you will not have control over what nodes we contact in a
lookup/announce. Over time, we will get better at making sure our clients dont get banned, but to do your part, do not send an excessive
amount of lookups/announces for the same info hash in a short period of time. Symptoms of getting banned include receiving less and less
contacts back when doing a search for an info hash. If you feel you have gotten banned, you can always restart the DHT since all nodes
(should) treat the (node id, source address) as the unique identifier for nodes and we always get a new node id on startup.

- **Sloppy DHT**: The kademlia DHT is also referred to as a sloppy DHT. This means that you will be able to find most (if not all)
nodes that announce for a given info hash. To make your applications more robust (and this is what torrent clients do), you should
develop a mechanism for receiving the contact information for other peers from peers themselves. This means that if you had two
segmented swarms of peers, only one person from one swarm has to be aware of one person from another swarm in order to join the
two swarms so that everyone knows of everyone else.
