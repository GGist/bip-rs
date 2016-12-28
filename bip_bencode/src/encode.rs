use bencode::{Bencode, BencodeKind};
use dictionary::Dictionary;

use std::iter::Extend;

pub fn encode<'a>(val: &Bencode<'a>) -> Vec<u8> {
    match val.kind() {
        BencodeKind::Int(n) => encode_int(n),
        BencodeKind::Bytes(n) => encode_bytes(&n),
        BencodeKind::List(n) => encode_list(n),
        BencodeKind::Dict(n) => encode_dict(n),
    }
}

fn encode_int(val: i64) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();

    bytes.push(::INT_START);
    bytes.extend(val.to_string().into_bytes());
    bytes.push(::BEN_END);

    bytes
}

fn encode_bytes(list: &[u8]) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();

    bytes.extend(list.len().to_string().into_bytes());
    bytes.push(::BYTE_LEN_END);
    bytes.extend(list.iter().map(|n| *n));

    bytes
}

fn encode_list<'a>(list: &[Bencode<'a>]) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();

    bytes.push(::LIST_START);
    for i in list {
        bytes.extend(encode(i));
    }
    bytes.push(::BEN_END);

    bytes
}

fn encode_dict<'a>(dict: &Dictionary<'a, Bencode<'a>>) -> Vec<u8> {
    // Need To Sort The Keys In The Map Before Encoding
    let mut bytes: Vec<u8> = Vec::new();

    let mut sort_dict = dict.to_list();
    sort_dict.sort_by(|&(a, _), &(b, _)| a.cmp(b));

    bytes.push(::DICT_START);
    // Iterate And Dictionary Encode The (String, Bencode) Pairs
    for &(ref key, ref value) in sort_dict.iter() {
        bytes.extend(encode_bytes(key));
        bytes.extend(encode(*value));
    }
    bytes.push(::BEN_END);

    bytes
}
