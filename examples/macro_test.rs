#[macro_use]
extern crate redox;

use std::net::{UdpSocket};

use redox::bencode::{DecodeBencode, Bencode, EncodeBencode};

fn main() {
    let mut recv_buffer = [0u8; 200];
    let dht_msg = ben_map!{
        "t" => ben_bytes!("ad"),
        "y" => ben_bytes!("q"),
        
        "q" => ben_bytes!("ping") ,
        "a" => ben_map!{
            "id" => ben_bytes!("abdjchdjskdleorituah")
        }
    };
    let udp = UdpSocket::bind("0.0.0.0:0").unwrap();
    
    udp.send_to(&dht_msg.encode()[..], "212.129.33.50:6881").unwrap();
    let (len, _) = udp.recv_from(&mut recv_buffer[..]).unwrap();
    
    let bencode = Bencode::decode(&recv_buffer[0..len]).unwrap();
    println!("{:?}", bencode);
}
