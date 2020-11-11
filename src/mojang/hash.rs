// Modified version of file from gist https://gist.github.com/RoccoDev/8fa130f1946f89702f799f89b8469bc9 by RoccoDev

extern crate crypto; // Tested with 0.2.36
extern crate rustc_serialize; // Tested with ^0.3

use crypto::digest::Digest;
use crypto::sha1::Sha1;

use std::iter;

use rustc_serialize::hex::ToHex;

pub fn calc_hash(server_id: &str, shared_secret: &[u8], public_key: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.input(server_id.as_bytes());
    hasher.input(shared_secret);
    hasher.input(public_key);
    let mut hex: Vec<u8> = iter::repeat(0).take((hasher.output_bits() + 7)/8).collect();
    hasher.result(&mut hex);

    let negative = (hex[0] & 0x80) == 0x80;

    if negative {
        two_complement(&mut hex);
        format!("-{}", remove_zeros(hex))
    }
    else {
        remove_zeros(hex)
    }
}

fn remove_zeros(hex: Vec<u8>) -> String {
    let mut as_string = hex.as_slice().to_hex();
    while as_string.starts_with("0") {
        as_string = as_string.strip_prefix("0").unwrap().to_string();
    }
    as_string
}

fn two_complement(bytes: &mut Vec<u8>) {
    let mut carry = true;
    for i in (0..bytes.len()).rev() {
        bytes[i] = !bytes[i] & 0xff;
        if carry {
            carry = bytes[i] == 0xff;
            bytes[i] = bytes[i] + 1;
        }
    }
}
