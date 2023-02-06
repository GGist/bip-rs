// #![feature(test)]

// extern crate bip_bencode;
// extern crate test;

// #[cfg(test)]
// mod benches {
//     use bip_bencode::{BencodeRef, BDecodeOpt};
//     use test::Bencher;

//     #[bench]
//     fn bench_nested_lists(b: &mut Bencher) {
//         let bencode = b"lllllllllllllllllllllllllllllllllllllllllllllllllleeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

//         b.iter(|| BencodeRef::decode(&bencode[..], BDecodeOpt::new(50, true, true)).unwrap());
//     }

//     #[bench]
//     fn bench_multi_kb_bencode(b: &mut Bencher) {
//         let bencode = include_bytes!("multi_kb.bencode");

//         b.iter(|| BencodeRef::decode(&bencode[..], BDecodeOpt::default()).unwrap());
//     }
// }
