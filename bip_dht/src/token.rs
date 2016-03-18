use std::net::{Ipv4Addr, Ipv6Addr};

use chrono::{DateTime, Duration, UTC};
use rand::{self};

use bip_util::error::{LengthResult, LengthError, LengthErrorKind};
use bip_util::convert::{self};
use bip_util::sha::{self, ShaHash};
use bip_util::net::{IpAddr};

/// We will partially follow the bittorrent implementation for issuing tokens to nodes, the
/// secret will change every 10 minutes and tokens up to 10 minutes old will be accepted. This
/// is in contrast with the bittorrent implementation where the secret changes every 5 minutes
/// and tokens up to 10 minutes old are accepted. Updating of the token will take place lazily.
/// However, with our implementation we are not going to store tokens that we have issued, instead,
/// store the secret and check if the token they gave us is valid for the current or last secret.
/// This is technically not what we want, but it will have essentially the same result when we
/// assume that nobody other than us knows the secret.

/// With this scheme we can guarantee that the minimum amount of time a token can be valid for
/// is the maximum amount of time a token is valid for in bittorrent in order to provide interop.
/// Since we arent storing the tokens we generate (which is awesome) we CANT track how long each
/// individual token has been checked out from the store and so each token is valid for some time
/// between 10 and 20 minutes in contrast with 5 and 10 minutes.

const REFRESH_INTERVAL_MINS: i64 = 10;

const IPV4_SECRET_BUFFER_LEN: usize = 4 + 4;
const IPV6_SECRET_BUFFER_LEN: usize = 16 + 4;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Token {
    token: [u8; sha::SHA_HASH_LEN]
}

impl Token {
    pub fn new(bytes: &[u8]) -> LengthResult<Token> {
        if bytes.len() != sha::SHA_HASH_LEN {
            Err(LengthError::new(LengthErrorKind::LengthExpected, sha::SHA_HASH_LEN))
        } else {
            let mut token = [0u8; sha::SHA_HASH_LEN];
            
            for (src, dst) in bytes.iter().zip(token.iter_mut()) {
                *dst = *src;
            }
            Ok(Token::from(token))
        }
    }
}

impl Into<[u8; sha::SHA_HASH_LEN]> for Token {
    fn into(self) -> [u8; sha::SHA_HASH_LEN] {
        self.token
    }
}

impl From<[u8; sha::SHA_HASH_LEN]> for Token {
    fn from(token: [u8; sha::SHA_HASH_LEN]) -> Token {
        Token{ token: token }
    }
}

impl AsRef<[u8]> for Token {
    fn as_ref(&self) -> &[u8] {
        &self.token
    }
}

//----------------------------------------------------------------------------//

#[derive(Copy, Clone)]
pub struct TokenStore {
    curr_secret:  u32,
    last_secret:  u32,
    last_refresh: DateTime<UTC>
}

impl TokenStore {
    pub fn new() -> TokenStore {
        // We cant just use a placeholder for the last secret as that would allow external
        // nodes to exploit recently started dhts. Instead, just generate another placeholder
        // secret for the last secret with the assumption that we wont get a valid announce
        // under that secret. We could go the option route but that isnt as clean.
        let curr_secret = rand::random::<u32>();
        let last_secret = rand::random::<u32>();
        let last_refresh = UTC::now();
        
        TokenStore{ curr_secret: curr_secret, last_secret: last_secret, last_refresh: last_refresh }
    }
    
    pub fn checkout(&mut self, addr: IpAddr) -> Token {
        self.refresh_check();
        
        generate_token_from_addr(addr, self.curr_secret)
    }
    
    pub fn checkin(&mut self, addr: IpAddr, token: Token) -> bool {
        self.refresh_check();
        
        validate_token_from_addr(addr, token, self.curr_secret, self.last_secret)
    }
    
    fn refresh_check(&mut self) {
        match intervals_passed(self.last_refresh) {
            0 => (),
            1 => {
                self.last_secret = self.curr_secret;
                self.curr_secret = rand::random::<u32>();
                self.last_refresh = UTC::now();
            },
            _ => {
                self.last_secret = rand::random::<u32>();
                self.curr_secret = rand::random::<u32>();
                self.last_refresh = UTC::now();
            }
        };
    }
}

/// Since we are lazily generating tokens, more than one interval could have passed since
/// we last generated a token in which case our last secret AND current secret could be
/// invalid.
///
/// Returns the number of intervals that have passed since the last refresh time.
fn intervals_passed(last_refresh: DateTime<UTC>) -> i64 {
    let refresh_duration = Duration::minutes(REFRESH_INTERVAL_MINS);
    let curr_time = UTC::now();
    
    let diff_time = curr_time - last_refresh;
    
    diff_time.num_minutes() / refresh_duration.num_minutes()
}

/// Generate a token from an ip address and a secret.
fn generate_token_from_addr(addr: IpAddr, secret: u32) -> Token {
    match addr {
        IpAddr::V4(v4) => generate_token_from_addr_v4(v4, secret),
        IpAddr::V6(v6) => generate_token_from_addr_v6(v6, secret)
    }
}

/// Generate a token from an ipv4 address and a secret.
fn generate_token_from_addr_v4(v4_addr: Ipv4Addr, secret: u32) -> Token {
    let mut buffer = [0u8; IPV4_SECRET_BUFFER_LEN];
    let v4_bytes = convert::ipv4_to_bytes_be(v4_addr);
    let secret_bytes = convert::four_bytes_to_array(secret);
    
    let source_iter = v4_bytes.iter().chain(secret_bytes.iter());
    for (dst, src) in buffer.iter_mut().zip(source_iter) {
        *dst = *src;
    }
    
    let hash_buffer = ShaHash::from_bytes(&buffer);
    Into::<[u8; sha::SHA_HASH_LEN]>::into(hash_buffer).into()
}

/// Generate a token from an ipv6 address and a secret.
fn generate_token_from_addr_v6(v6_addr: Ipv6Addr, secret: u32) -> Token {
    let mut buffer = [0u8; IPV6_SECRET_BUFFER_LEN];
    let v6_bytes = convert::ipv6_to_bytes_be(v6_addr);
    let secret_bytes = convert::four_bytes_to_array(secret);
    
    let source_iter = v6_bytes.iter().chain(secret_bytes.iter());
    for (dst, src) in buffer.iter_mut().zip(source_iter) {
        *dst = *src;
    }
    
    let hash_buffer = ShaHash::from_bytes(&buffer);
    Into::<[u8; sha::SHA_HASH_LEN]>::into(hash_buffer).into()
}

/// Validate a token given an ip address and the two current secrets.
fn validate_token_from_addr(addr: IpAddr, token: Token, secret_one: u32, secret_two: u32) -> bool {
    match addr {
        IpAddr::V4(v4) => {
            validate_token_from_addr_v4(v4, token, secret_one) || validate_token_from_addr_v4(v4, token, secret_two)
        },
        IpAddr::V6(v6) => {
            validate_token_from_addr_v6(v6, token, secret_one) || validate_token_from_addr_v6(v6, token, secret_two)
        }
    }
}

/// Validate a token given an ipv4 address and one secret.
fn validate_token_from_addr_v4(v4_addr: Ipv4Addr, token: Token, secret: u32) -> bool {
    generate_token_from_addr_v4(v4_addr, secret) == token
}

/// Validate a token given an ipv6 address and one secret.
fn validate_token_from_addr_v6(v6_addr: Ipv6Addr, token: Token, secret: u32) -> bool {
    generate_token_from_addr_v6(v6_addr, secret) == token
}

#[cfg(test)]
mod tests {
    use chrono::{Duration};
    use bip_util::test as bip_test;

    use token::{TokenStore};
    
    #[test]
    fn positive_accept_valid_v4_token() {
        let mut store = TokenStore::new();
        let v4_addr = bip_test::dummy_ipv4_addr();
        
        let valid_token = store.checkout(v4_addr);
        
        assert!(store.checkin(v4_addr, valid_token));
    }
    
    #[test]
    fn positive_accept_valid_v6_token() {
        let mut store = TokenStore::new();
        let v6_addr = bip_test::dummy_ipv6_addr();
        
        let valid_token = store.checkout(v6_addr);
        
        assert!(store.checkin(v6_addr, valid_token));
    }
    
    #[test]
    fn positive_accept_v4_token_from_second_secret() {
        let mut store = TokenStore::new();
        let v4_addr = bip_test::dummy_ipv4_addr();
        
        let valid_token = store.checkout(v4_addr);
        
        let past_offset = Duration::minutes((super::REFRESH_INTERVAL_MINS * 2) - 1);
        let past_time = bip_test::travel_into_past(past_offset);
        store.last_refresh = past_time;
        
        assert!(store.checkin(v4_addr, valid_token));
    }
    
    #[test]
    fn positive_accept_v6_token_from_second_secret() {
        let mut store = TokenStore::new();
        let v6_addr = bip_test::dummy_ipv6_addr();
        
        let valid_token = store.checkout(v6_addr);
        
        let past_offset = Duration::minutes((super::REFRESH_INTERVAL_MINS * 2) - 1);
        let past_time = bip_test::travel_into_past(past_offset);
        store.last_refresh = past_time;
        
        assert!(store.checkin(v6_addr, valid_token));
    }
    
    #[test]
    #[should_panic]
    fn negative_reject_expired_v4_token() {
        let mut store = TokenStore::new();
        let v4_addr = bip_test::dummy_ipv4_addr();
        
        let valid_token = store.checkout(v4_addr);
        
        let past_offset = Duration::minutes(super::REFRESH_INTERVAL_MINS * 2);
        let past_time = bip_test::travel_into_past(past_offset);
        store.last_refresh = past_time;
        
        assert!(store.checkin(v4_addr, valid_token));
    }
    
    #[test]
    #[should_panic]
    fn negative_reject_expired_v6_token() {
        let mut store = TokenStore::new();
        let v6_addr = bip_test::dummy_ipv6_addr();
        
        let valid_token = store.checkout(v6_addr);
        
        let past_offset = Duration::minutes(super::REFRESH_INTERVAL_MINS * 2);
        let past_time = bip_test::travel_into_past(past_offset);
        store.last_refresh = past_time;
        
        assert!(store.checkin(v6_addr, valid_token));
    }
}