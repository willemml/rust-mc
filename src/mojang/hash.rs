// Modified version of https://gist.github.com/RoccoDev/8fa130f1946f89702f799f89b8469bc9

use openssl::sha::Sha1;

/// Calculate hashes of Minecraft servers to authenticate with them and clients.
pub fn calc_hash(server_id: &str, shared_secret: &[u8], public_key: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(server_id.as_bytes());
    hasher.update(shared_secret);
    hasher.update(public_key);
    let mut hex = hasher.finish();

    let negative = (hex[0] & 0x80) == 0x80;

    if negative {
        two_complement(&mut hex);
        format!("-{}", remove_zeros(&hex))
    }
    else {
        remove_zeros(&hex)
    }
}

fn remove_zeros(hex: &[u8]) -> String {
    let mut as_string = hex::encode(hex);
    while as_string.starts_with("0") {
        as_string = as_string.strip_prefix("0").unwrap().to_string();
    }
    as_string
}

fn two_complement(bytes: &mut [u8]) {
    let mut carry = true;
    for i in (0..bytes.len()).rev() {
        bytes[i] = !bytes[i] & 0xff;
        if carry {
            carry = bytes[i] == 0xff;
            bytes[i] = bytes[i] + 1;
        }
    }
}
