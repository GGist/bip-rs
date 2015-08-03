use bencode::{self, BencodeView, BencodeKind};
use util::{Dictionary};

use std::iter::Extend;

pub fn encode<'a, T>(val: T) -> Vec<u8> where T: BencodeView<'a> {
    match val.kind() {
        BencodeKind::Int(n)   => encode_int(n),
        BencodeKind::Bytes(n) => encode_bytes(&n),
        BencodeKind::List(n)  => encode_list(n),
        BencodeKind::Dict(n)  => encode_dict(n)
    }
}

fn encode_int(val: i64) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();

    bytes.push(bencode::INT_START);
    bytes.extend(val.to_string().as_bytes());
    bytes.push(bencode::BEN_END);

    bytes
}

fn encode_bytes(list: &[u8]) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();

    bytes.extend(list.len().to_string().as_bytes());
    bytes.push(bencode::BYTE_LEN_END);
    bytes.extend(list);

    bytes
}

fn encode_list<'a, T>(list: &[T]) -> Vec<u8> where T: BencodeView<'a> {
    let mut bytes: Vec<u8> = Vec::new();

    bytes.push(bencode::LIST_START);
    for i in list {
        bytes.extend(&encode(i));
    }
    bytes.push(bencode::BEN_END);

    bytes
}

fn encode_dict<'a, T>(dict: &Dictionary<'a, T>) -> Vec<u8>
    where T: BencodeView<'a> {
    // Need To Sort The Keys In The Map Before Encoding
    let mut bytes: Vec<u8> = Vec::new();

    let mut sort_dict = dict.to_list();
    sort_dict.sort_by(|&(a, _), &(b, _)| a.cmp(b));

    bytes.push(bencode::DICT_START);
    // Iterate And Dictionary Encode The (String, Bencode) Pairs
    for &(ref key, ref value) in sort_dict.iter() {
        bytes.extend(&encode_bytes(key.as_bytes()));
        bytes.extend(&encode(*value));
    }
    bytes.push(bencode::BEN_END);

    bytes
}
