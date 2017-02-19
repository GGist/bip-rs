use access::bencode::{BencodeRefKind, BRefAccess};
use access::dict::BDictAccess;
use access::list::BListAccess;

use std::iter::Extend;

pub fn encode<'a, T>(val: T, bytes: &mut Vec<u8>) where T: BRefAccess<'a> {
    match val.kind() {
        BencodeRefKind::Int(n)  => encode_int(n, bytes),
        BencodeRefKind::Bytes(n) => encode_bytes(&n, bytes),
        BencodeRefKind::List(n) => encode_list(n, bytes),
        BencodeRefKind::Dict(n) => encode_dict(n, bytes),
    }
}

fn encode_int(val: i64, bytes: &mut Vec<u8>) {
    bytes.push(::INT_START);

    bytes.extend(val.to_string().into_bytes());

    bytes.push(::BEN_END);
}

fn encode_bytes(list: &[u8], bytes: &mut Vec<u8>) {
    bytes.extend(list.len().to_string().into_bytes());

    bytes.push(::BYTE_LEN_END);

    bytes.extend(list.iter().map(|n| *n));
}

fn encode_list<'a, T>(list: &BListAccess<T>, bytes: &mut Vec<u8>) where T: BRefAccess<'a> {
    bytes.push(::LIST_START);

    for i in list {
        encode(i, bytes);
    }

    bytes.push(::BEN_END);
}

fn encode_dict<'a, T>(dict: &BDictAccess<'a, T>, bytes: &mut Vec<u8>) where T: BRefAccess<'a> {
    // Need To Sort The Keys In The Map Before Encoding
    let mut sort_dict = dict.to_list();
    sort_dict.sort_by(|&(a, _), &(b, _)| a.cmp(b));

    bytes.push(::DICT_START);
    // Iterate And Dictionary Encode The (String, Bencode) Pairs
    for &(ref key, ref value) in sort_dict.iter() {
        encode_bytes(key, bytes);
        encode(*value, bytes);
    }
    bytes.push(::BEN_END);
}
