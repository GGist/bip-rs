redox-rs
=======
A bittorrent library and client written in pure Rust.

Dashboard
---------
| Linux CI | Windows CI | Test Coverage | Crate | Documentation |
|:--------:|:----------:|:-------------:|:---------:|:-------------:|:-------:|
| [![Build Status](https://travis-ci.org/GGist/redox-rs.svg?branch=master)](https://travis-ci.org/GGist/redox-rs) | [![Build status](https://ci.appveyor.com/api/projects/status/vwp832w2u745aa1u/branch/master?svg=true)](https://ci.appveyor.com/project/GGist/redox-rs/branch/master) | [![Coverage Status](https://coveralls.io/repos/GGist/redox-rs/badge.svg)](https://coveralls.io/r/GGist/redox-rs) |  | [![Docs](https://img.shields.io/badge/docs-in--progress-blue.svg)](http://ggist.github.io/redox-rs/index.html)

Currently Redesigning/Updating Code For Stable Rust
---------------------------------------------------
~~Roadmap~~
-------
**Core:**
* ~~Decoding & Encoding For Bencode~~
* ~~Unpacking Of Torrent File Fields From Bencode~~
* ~~UDP Tracker Protocol~~
	* ~~Find Local IPv4 Interface~~
	* ~~Implement UPnP Support For Port Forwarding~~
		* ~~Discovery Mechanism Over UDP~~
		* ~~WANIPConnection SOAP Protocol For Setting Up Forward~~
	* ~~Finish Up Interface For Tracker Communication~~
* ~~Implement Algorithm For Peer Wire Protocol~~
	* ~~Decide On An Async Or Sync API~~
	* ~~Piece Selection Strategy~~ *Implemented By Client*
	* ~~Chocking/Interested Primitives~~
	* ~~Piece Verification Routines~~
	* ~~End Game Algorithm~~ *Implemented By Client*
* Unit Test Everything!!!
* DRY Up All Modules That Have Passed Unit Testing
* Extract UPnP Module Into Separate Crate
* Build Reference Client

**Extras:**
* Implement DHT Protocol
    * Bootstrap From uTorrent Server
    * Bootstrap From Popular Torrent
    * Add Caching Mechanism
* Implement NAT PMP Protocol
* Look In To NAT Punch-through

**Informative Links:**
* https://wiki.theory.org/BitTorrentSpecification
* https://code.google.com/p/udpt/wiki/UDPTrackerProtocol
* http://www.kristenwidman.com/blog/how-to-write-a-bittorrent-client-part-1/
