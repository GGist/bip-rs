redox-rs
========
A full featured bittorrent library written in Rust.

Dashboard
---------
| Linux CI | Windows CI | Test Coverage | Crate | Documentation |
|:--------:|:----------:|:-------------:|:---------:|:-------------:|:-------:|
| [![Build Status](https://travis-ci.org/GGist/redox-rs.svg?branch=master)](https://travis-ci.org/GGist/redox-rs) | [![Build status](https://ci.appveyor.com/api/projects/status/vwp832w2u745aa1u/branch/master?svg=true)](https://ci.appveyor.com/project/GGist/redox-rs/branch/master) | [![Coverage Status](https://coveralls.io/repos/GGist/redox-rs/badge.svg?branch=master)](https://coveralls.io/r/GGist/redox-rs?branch=master) | [![Crate](http://meritbadge.herokuapp.com/redox)](https://crates.io/crates/redox) | [![Docs](https://img.shields.io/badge/docs-in--progress-blue.svg)](http://ggist.github.io/redox-rs/index.html)

Currently Redesigning/Updating Code For Stable Rust
---------------------------------------------------
Bittorrent Enhancement Proposals (BEP) Supported
-------------------------------------------------
- [ ] BEP 03: [The BitTorrent Protocol Specification](http://www.bittorrent.org/beps/bep_0003.html)
- [ ] BEP 04: [Known Number Allocations](http://www.bittorrent.org/beps/bep_0004.html)
- [ ] BEP 05: [DHT Protocol](http://www.bittorrent.org/beps/bep_0005.html)
- [ ] BEP 06: [Fast Extension](http://www.bittorrent.org/beps/bep_0006.html)
- [ ] BEP 07: [IPv6 Tracker Extension](http://www.bittorrent.org/beps/bep_0007.html)
- [ ] BEP 09: [Extension for Peers to Send Metadata Files](http://www.bittorrent.org/beps/bep_0009.html)
- [ ] BEP 10: [Extension Protocol](http://www.bittorrent.org/beps/bep_0010.html)
- [ ] BEP 12: [Multitracker Metadata Extension](http://www.bittorrent.org/beps/bep_0012.html)
- [ ] BEP 15: [UDP Tracker Protocol](http://www.bittorrent.org/beps/bep_0015.html)
- [ ] BEP 16: [Superseeding](http://www.bittorrent.org/beps/bep_0016.html)
- [ ] BEP 17: [HTTP Seeding (Hoffman-style)](http://www.bittorrent.org/beps/bep_0017.html)
- [ ] BEP 18: [Search Engine Specification](http://www.bittorrent.org/beps/bep_0018.html)
- [ ] BEP 19: [HTTP/FTP Seeding (GetRight-style)](http://www.bittorrent.org/beps/bep_0019.html)
- [ ] BEP 20: [Peer ID Conventions](http://www.bittorrent.org/beps/bep_0020.html)
- [ ] BEP 21: [Extension for Partial Seeds](http://www.bittorrent.org/beps/bep_0021.html)
- [ ] BEP 22: [BitTorrent Local Tracker Discovery Protocol](http://www.bittorrent.org/beps/bep_0022.html)
- [ ] BEP 23: [Tracker Returns Compact Peer Lists](http://www.bittorrent.org/beps/bep_0023.html)
- [ ] BEP 24: [Tracker Returns External IP](http://www.bittorrent.org/beps/bep_0024.html)
- [ ] BEP 26: [Zeroconf Peer Advertising and Discovery](http://www.bittorrent.org/beps/bep_0026.html)
- [ ] BEP 27: [Private Torrents](http://www.bittorrent.org/beps/bep_0027.html)
- [ ] BEP 28: [Tracker exchange](http://www.bittorrent.org/beps/bep_0028.html)
- [ ] BEP 29: [uTorrent transport protocol](http://www.bittorrent.org/beps/bep_0029.html)
- [ ] BEP 30: [Merkle tree torrent extension](http://www.bittorrent.org/beps/bep_0030.html)
- [ ] BEP 31: [Tracker Failure Retry Extension](http://www.bittorrent.org/beps/bep_0031.html)
- [ ] BEP 32: [IPv6 extension for DHT](http://www.bittorrent.org/beps/bep_0032.html)
- [ ] BEP 33: [DHT scrape](http://www.bittorrent.org/beps/bep_0033.html)
- [ ] BEP 34: [DNS Tracker Preferences](http://www.bittorrent.org/beps/bep_0034.html)
- [ ] BEP 35: [Torrent Signing](http://www.bittorrent.org/beps/bep_0035.html)
- [ ] BEP 36: [Torrent RSS feeds](http://www.bittorrent.org/beps/bep_0036.html)
- [ ] BEP 38: [Finding Local Data Via Torrent File Hints](http://www.bittorrent.org/beps/bep_0038.html)
- [ ] BEP 39: [Updating Torrents Via Feed URL](http://www.bittorrent.org/beps/bep_0039.html)
- [ ] BEP 40: [Canonical Peer Priority](http://www.bittorrent.org/beps/bep_0040.html)
- [ ] BEP 41: [UDP Tracker Protocol Extensions](http://www.bittorrent.org/beps/bep_0041.html)
- [ ] BEP 42: [DHT Security Extension](http://www.bittorrent.org/beps/bep_0042.html)
- [ ] BEP 43: [Read-only DHT Nodes](http://www.bittorrent.org/beps/bep_0043.html)
- [ ] BEP 44: [Storing arbitrary data in the DHT](http://www.bittorrent.org/beps/bep_0044.html)

**Informative Links:**
* https://wiki.theory.org/BitTorrentSpecification
* https://code.google.com/p/udpt/wiki/UDPTrackerProtocol
* http://www.kristenwidman.com/blog/how-to-write-a-bittorrent-client-part-1/
