use crate::access::bencode::{BRefAccess, BencodeRefKind};
use crate::access::dict::BDictAccess;
use crate::access::list::BListAccess;

use std::iter::Extend;

pub fn encode<T>(val: T, bytes: &mut Vec<u8>)
where
    T: BRefAccess,
    T::BKey: AsRef<[u8]>,
{
    match val.kind() {
        BencodeRefKind::Int(n) => encode_int(n, bytes),
        BencodeRefKind::Bytes(n) => encode_bytes(&n, bytes),
        BencodeRefKind::List(n) => encode_list(n, bytes),
        BencodeRefKind::Dict(n) => encode_dict(n, bytes),
    }
}

fn encode_int(val: i64, bytes: &mut Vec<u8>) {
    bytes.push(crate::INT_START);

    bytes.extend(val.to_string().into_bytes());

    bytes.push(crate::BEN_END);
}

fn encode_bytes(list: &[u8], bytes: &mut Vec<u8>) {
    bytes.extend(list.len().to_string().into_bytes());

    bytes.push(crate::BYTE_LEN_END);

    bytes.extend(list.iter().copied());
}

fn encode_list<T>(list: &dyn BListAccess<T>, bytes: &mut Vec<u8>)
where
    T: BRefAccess,
    T::BKey: AsRef<[u8]>,
{
    bytes.push(crate::LIST_START);

    for i in list {
        encode(i, bytes);
    }

    bytes.push(crate::BEN_END);
}

fn encode_dict<'a, K, V>(dict: &dyn BDictAccess<K, V>, bytes: &mut Vec<u8>)
where
    K: AsRef<[u8]>,
    V: BRefAccess,
    V::BKey: AsRef<[u8]>,
{
    // Need To Sort The Keys In The Map Before Encoding
    let mut sort_dict = dict.to_list();
    sort_dict.sort_by(|&(a, _), &(b, _)| a.as_ref().cmp(b.as_ref()));

    bytes.push(crate::DICT_START);
    // Iterate And Dictionary Encode The (String, Bencode) Pairs
    for &(ref key, ref value) in sort_dict.iter() {
        encode_bytes(key.as_ref(), bytes);
        encode(*value, bytes);
    }
    bytes.push(crate::BEN_END);
}
