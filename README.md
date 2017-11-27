# Bittorrent Infrastructure Project
[![Build Status](https://travis-ci.org/GGist/bip-rs.svg?branch=master)](https://travis-ci.org/GGist/bip-rs) [![Build status](https://ci.appveyor.com/api/projects/status/muiqrh76k5hoir0s/branch/master?svg=true)](https://ci.appveyor.com/project/GGist/bip-rs/branch/master)

A collection of crates for building applications using bittorrent technologies.

## Bencode (bip_bencode) - [![Documentation](https://docs.rs/bip_bencode/badge.svg)](https://docs.rs/bip_bencode) [![Crate](http://meritbadge.herokuapp.com/bip_bencode)](https://crates.io/crates/bip_bencode)

**About**: Bencode is the binary encoding used throughout bittorrent technologies from metainfo files to DHT messages. Bencode types include integers, byte arrays, lists, and dictionarys, of which the last two can hold any bencode type (they could be recursively constructed).

## Disk (bip_disk) - [![Documentation](https://docs.rs/bip_disk/badge.svg)](https://docs.rs/bip_disk) [![Crate](http://meritbadge.herokuapp.com/bip_disk)](https://crates.io/crates/bip_disk)

**About**: Disk management is all about storing/loading pieces to/from any object implementing the ```FileSystem``` interface, which means torrent storage could be transparently sent to disk, stored in memory, pushed to a distributed file system, or even uploaded to the cloud as pieces come in. In addition, notifications are sent when good or bad pieces are detected as soon as enough blocks are sent to the disk manager that make up a full piece.

## Handshake (bip_handshake) - [![Documentation](https://docs.rs/bip_handshake/badge.svg)](https://docs.rs/bip_handshake) [![Crate](http://meritbadge.herokuapp.com/bip_handshake)](https://crates.io/crates/bip_handshake)

**About**: Handshaking is the process of connecting to a peer and exchanging information related to how a peer will be communicating with you and vice versa. In our case, there are many bittorrent technologies that could generally be considered peer discovery mechanisms (local peer discovery, dht, trackers, peer exchange) where once a peer is discovered, a client may want to immediately attempt to establish a connection via a handshake. This module provides a trait for custom handshake implementations, as well as the standard bittorrent handshake, so that clients can specify a handshaking mechanism for peer discovery services to forward contact information along to.

## Peer (bip_peer) - [![Documentation](https://docs.rs/bip_peer/badge.svg)](https://docs.rs/bip_peer) [![Crate](http://meritbadge.herokuapp.com/bip_peer)](https://crates.io/crates/bip_peer)

**About**: Communicating with bittorrent peers involves choking (telling someone we won't respond to them now), expressing interest (telling someone, if we were unchoked, we would be interested in some data they hold), as well as downloading and uploading blocks to peers. This package defines some common bittorrent messages, including those as part of the `ExtensionBits` in `bip_handshake`, as well as those included in the [extension protocol](http://www.bittorrent.org/beps/bep_0010.html). We also provide a `PeerManager` for heartbeating peers and multiplexing messages sent to/from peers so that clients have an easier time communicating asynchronously with many peers.

## Select (bip_select) - [![Documentation](https://docs.rs/bip_select/badge.svg)](https://docs.rs/bip_select) [![Crate](http://meritbadge.herokuapp.com/bip_select)](https://crates.io/crates/bip_select)

**About**: Selection is broken up in to three classes of algorithms. First, we have *Piece Revelation* which is focused on determining which pieces we should reveal (even if we don't have the piece...) and to whom. Second, we have *Piece Selection* which is focused on which pieces we should download/upload next. Third, we have *Piece Queueing* which is, given a piece we want to download, which peers should we send such a request to. We can mix and match different algorithms to create a swarm that may have different characteristics than other swarms.

## Mainline DHT (bip_dht) - [![Documentation](https://docs.rs/bip_dht/badge.svg)](https://docs.rs/bip_dht) [![Crate](http://meritbadge.herokuapp.com/bip_dht)](https://crates.io/crates/bip_dht)

**About**: The Mainline DHT is used by bittorrent to distribute contact information for peers interested in specified files. More generally, any application can use the Mainline DHT to discover peers in a distributed and decentralized fashion. You can take advantage of the DHT as long as your application has a way of exposing interest in other peers via a SHA-1 hash (20 byte value).

**Example**: For example, a decentralized chat application could ask the user for a room name to join, you could then hash that room name and search for other peers using that application who are also interested in joining the same room.

## Metainfo (bip_metainfo) - [![Documentation](https://docs.rs/bip_metainfo/badge.svg)](https://docs.rs/bip_metainfo) [![Crate](http://meritbadge.herokuapp.com/bip_metainfo)](https://crates.io/crates/bip_metainfo)

**About**: At their core, metainfo files serve the purpose of providing a list of checksums for clients interested in specific files, how long each hashed piece should be, and the directory structure for the files. This allows clients to verify the integrity of received files, as well as the ability to recreate exactly the directory structure for the files. Aside from that, there is a plethora of optional information that can be included in this file such as nodes to be contacted in the DHT, trackers to contact, as well as comments, date created, who created the metainfo file, etc.

## Tracker (bip_utracker) - [![Documentation](https://docs.rs/bip_utracker/badge.svg)](https://docs.rs/bip_utracker) [![Crate](http://meritbadge.herokuapp.com/bip_utracker)](https://crates.io/crates/bip_utracker)

**About**: Trackers provide a centralized solution to peer discovery within the bittorrent eco-system. Clients send messages to a specific set of trackers, updating them with any state changes that have occured pertaining to the download of files. However, using the start and stop events we can use trackers generically to either add or remove ourselves from a tracker for the purposes of peer discovery for any application.

## References

* Official Specifications: http://www.bittorrent.org/beps/bep_0000.html
* Readable Specification: https://wiki.theory.org/index.php/BitTorrentSpecification
* Other Interesting Papers
    * Piece Related Algorithms: http://ieeexplore.ieee.org/document/6409840/
    * Performance Modelling: https://arxiv.org/ftp/arxiv/papers/1311/1311.1195.pdf

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
