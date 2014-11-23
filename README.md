RustBT - [![Build Status](https://travis-ci.org/GGist/RustBT.svg)](https://travis-ci.org/GGist/RustBT) [![Documentation](http://img.shields.io/badge/docs-in--progress-blue.svg)](http://ggist.github.io/RustBT/rust-bt/index.html) [![License](http://img.shields.io/badge/license-Apache%202-red.svg)](https://raw.githubusercontent.com/GGist/RustBT/master/LICENSE)
=======
A BitTorrent library and client written in pure Rust.

Roadmap
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
* Implement Algorithm For Peer Wire Protocol
	* Decide On An Async Or Sync API
	* Piece Selection Strategy
	* Chocking/Interested Primitives
	* Piece Verification Routines
	* End Game Algorithm
    
**Extras:**
* Implement DHT Protocol
    * Bootstrap From uTorrent Server
    * Bootstrap From Popular Torrent
    * Add Caching Mechanism
* Implement NAT PMP Protocol
* Look In To NAT Punch-through
