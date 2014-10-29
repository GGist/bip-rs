use util;
use regex::Regex;
use std::path::BytesContainer;
use std::io::net::tcp::TcpStream;
use std::io::{IoResult, InvalidInput};
use std::io::net::udp::UdpSocket;
use std::io::net::ip::{SocketAddr, Ipv4Addr};
use std::io::File;

// http://upnp.org/sdcps-and-certification/standards/sdcps/
// http://www.upnp.org/specs/arch/UPnP-arch-DeviceArchitecture-v1.0-20080424.pdf

// {1} = Device Max Wait Period, {2} = Search Target
static DISCOVERY_ADDR: SocketAddr = SocketAddr{ ip: Ipv4Addr(239, 255, 255, 250), port: 1900 };
static GENERIC_SEARCH_REQUEST: &'static str = "M-SEARCH * HTTP/1.1\r
HOST: 239.255.255.250:1900\r
MAN: \"ssdp:discover\"\r
MX: {1}\r
ST: {2}\r\n\r\n";

// {1} = Path, {2} = IP Address:Port
static GENERIC_HTTP_REQUEST: &'static str = "GET {1} HTTP/1.1\r
Host: {2}\r\n\r\n";

// {1} = Control URL, {2} = IP Address:Port, {3} = Bytes In Payload,
// {4} = Search Target Of Service, {5} = Action Name, {6} = Action Parameters
static GENERIC_SOAP_REQUEST: &'static str = "POST {1} HTTP/1.1\r
HOST: {2}\r
CONTENT-LENGTH: {3}\r
CONTENT-TYPE: text/xml; charset=\"utf-8\"\r
SOAPACTION: \"{4}#{5}\"\r
\r
<?xml version=\"1.0\"?>
<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\"
s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">
    <s:Body>
        <u:{5} xmlns:u=\"{4}\">{6}</u:{5}>
    </s:Body>
</s:Envelope>";
// Does Not Include Replacement Identifiers Or Quote Escapes
static BASE_CONTENT_LENGTH: uint = 222;

// UPnPInterface Classifiers
static ROOT_REGEX: Regex = regex!(r"upnp:(rootdevice)");
static DEVICE_REGEX: Regex = regex!(r":device:(.+?):(\d+)");
static SERVICE_REGEX: Regex = regex!(r":service:(.+?):(\d+)");
static IDENTIFIER_REGEX: Regex = regex!(r"uuid:([\w\d-]+)");

// Case Insensitive Matches For Header Fields
static ST_REGEX: Regex = regex!(r"(?i)ST: *([^\r\n]+)");
static USN_REGEX: Regex = regex!(r"(?i)USN: *([^\r\n]+)");
static LOCATION_REGEX: Regex = regex!(r"(?i)Location: *([^\r\n]+)");

/// A tuple consisting of start and end position.
pub type StrPos = (uint, uint);

/// Type used to represent a service description. Service actions can be viewed
/// as well as related in and out parameters and service state variables.
pub struct ServiceDesc {
    location: SocketAddr,
    search_target: String,
    control_path: String,
    service_desc: String,
    actions: Vec<StrPos>,
    var_table: Option<StrPos>
}

impl ServiceDesc {
    /// Takes a location corresponding to the root device description page of a UPnPInterface
    /// as well as the st field of the interface which will always be unique to a service/device.
    ///
    /// This is a blocking operation.
    pub fn parse(location: &str, st: &str) -> IoResult<ServiceDesc> {
        // Setup Regex For Parsing Device Description Page
        let service_find = String::from_str("<service>\\s*?<serviceType>{1}</serviceType>(?:.|\n)+?</service>").replace("{1}", st);
        let service_regex = try!(Regex::new(service_find.as_slice()).or_else( |_|
            Err(util::get_error(InvalidInput, "Failed To Create Service Regex"))
        ));
        let scpd_find: Regex = regex!(r"<SCPDURL>(.+?)</SCPDURL>");
        let control_find: Regex = regex!(r"<controlURL>(.+?)</controlURL>");
        
        // Setup Regex For Parsing Service Description Page
        let action_regex: Regex = regex!(r"<action>(?:.|\n)+?</action>");
        let state_vars_regex: Regex = regex!(r"<serviceStateTable>(?:.|\n)+?</serviceStateTable>");
        
        // Setup Data To Query Device Description Page
        let path = try!(util::get_path(location));
        let sock_addr = try!(util::get_sockaddr(location));
        let request = String::from_str(GENERIC_HTTP_REQUEST).replace("{1}", path)
            .replace("{2}", sock_addr.to_string().as_slice());

        // Query Device Description Page
        let mut tcp_sock = try!(TcpStream::connect(sock_addr.ip.to_string().as_slice(), sock_addr.port));
        
        try!(tcp_sock.write_str(request.as_slice()));
        let response = try!(tcp_sock.read_to_string());
        
        let (tag_begin, tag_end) = try!(service_regex.find(response.as_slice()).ok_or(
            util::get_error(InvalidInput, "Could Not Find Service Tag")
        ));

        // Pull Service Description Path And Control Path
        let service_tag = response.as_slice().slice(tag_begin, tag_end);
        
        let scpd_path = try!(scpd_find.captures(service_tag).ok_or(
            util::get_error(InvalidInput, "Could Not Capture SCPD Path")
        )).at(1);
        let control_path = try!(control_find.captures(service_tag).ok_or(
            util::get_error(InvalidInput, "Could Not Capture Control Path")
        )).at(1);
        
        // Query Service Description
        tcp_sock = try!(TcpStream::connect(sock_addr.ip.to_string().as_slice(), sock_addr.port));
        let request = request.replace(path.as_slice(), scpd_path);

        try!(tcp_sock.write_str(request.as_slice()));
        let response = try!(tcp_sock.read_to_string());
        
        // Pull Out Actions And State Variable Table Locations
        let actions = action_regex.find_iter(response.as_slice()).collect::<Vec<(uint, uint)>>();
        let var_table = state_vars_regex.find(response.as_slice());
        
        Ok(ServiceDesc{ location: sock_addr, search_target: st.to_string(), 
            control_path: control_path.to_string(), service_desc: response, 
            actions: actions, var_table: var_table })
    }
    
    pub fn soap_request(&self, action: &str, params: &[(&str, &str)]) -> IoResult<()> {
        // Build Parameter Section Of Payload
        let mut arguments = String::from_str("");
        
        for &(tag, value) in params.iter() {
            arguments.push_str("<");
            arguments.push_str(tag);
            arguments.push_str(">");
            
            arguments.push_str(value);
            
            arguments.push_str("</");
            arguments.push_str(tag);
            arguments.push_str(">");
        }
        
        // Substitute In Parameters
        let content_len = BASE_CONTENT_LENGTH + arguments.len() + 
            (action.len() * 2) + self.search_target.len();
        let request = GENERIC_SOAP_REQUEST.replace("{1}", self.control_path.as_slice())
            .replace("{2}", self.location.to_string().as_slice())
            .replace("{3}", content_len.to_string().as_slice())
            .replace("{4}", self.search_target.as_slice())
            .replace("{5}", action)
            .replace("{6}", arguments.as_slice());
            println!("{}", request);
            let request = File::open(&Path::new("spike/data.txt")).read_to_string();
        // Send Request
        println!("{}", request);
        let mut tcp_sock = try!(TcpStream::connect("192.168.1.1", 1780));
        
        try!(tcp_sock.write_str(request.unwrap().as_slice()));
        let response = try!(tcp_sock.read_to_string());
        
        println!("{}", response.len());
        
        Ok(())
    }
}

/// Type used to represent different interfaces available on a network as defined 
/// by the UPnP specification. This type can be used to get very general information
/// about an interface but also to get access to the services exposed by the interface.
pub enum UPnPInterface {
    Root(String, StrPos, StrPos, StrPos, StrPos, StrPos),
    Device(String, StrPos, StrPos, StrPos, StrPos, StrPos),
    Service(String, StrPos, StrPos, StrPos, StrPos, StrPos),
    Identifier(String, StrPos, StrPos, StrPos, StrPos, StrPos)
}

impl UPnPInterface {
    /// Sends out a search request for all UPnP interfaces on the specified from_addr.
    ///
    /// This is a blocking operation.
    pub fn find_all(from_addr: SocketAddr) -> IoResult<Vec<UPnPInterface>> {
        let request = String::from_str(GENERIC_SEARCH_REQUEST).replace("{1}", "5").replace("{2}", "ssdp:all");
        let replies = try!(send_search(from_addr, 5500, request.as_slice()));
        
        parse_interfaces(replies)
    }
    
    /// Sends out a search request for all UPnP services on the specified from_addr. 
    ///
    /// This function does not check to make sure that only services responded
    /// to our request, so it is assumed that devices on the network correctly
    /// follow the UPnP specification. As of now, any services that are not defined
    /// at http://upnp.org/sdcps-and-certification/standards/sdcps/ will not be found
    /// by this method (use find_all and filter with is_service).
    ///
    /// This is a blocking operation.
    pub fn find_services(from_addr: SocketAddr, name: &str, version: &str) -> IoResult<Vec<UPnPInterface>> {
        let service_st = String::from_str("urn:schemas-upnp-org:service:{1}:{2}").replace("{1}", name).replace("{2}", version);
        let request = String::from_str(GENERIC_SEARCH_REQUEST).replace("{1}", "5").replace("{2}", service_st.as_slice());
        
        let replies = try!(send_search(from_addr, 5500, request.as_slice()));
        
        parse_interfaces(replies)
    }
            
    /// Sends out a search request for all UPnP devices on the specified from_addr. 
    ///
    /// This function does not check to make sure that only devices responded
    /// to our request, so it is assumed that devices on the network correctly
    /// follow the UPnP specification. As of now, any devices that are not defined
    /// at http://upnp.org/sdcps-and-certification/standards/sdcps/ will not be found
    /// by this method (use find_all and filter with is_device).
    ///
    /// This is a blocking operation.
    pub fn find_devices(from_addr: SocketAddr, name: &str, version: &str) -> IoResult<Vec<UPnPInterface>> {
        let device_st = String::from_str("urn:schemas-upnp-org:device:{1}:{2}").replace("{1}", name).replace("{2}", version);
        let request = GENERIC_SEARCH_REQUEST.replace("{1}", "5").replace("{2}", device_st.as_slice());
        
        let replies = try!(send_search(from_addr, 5500, request.as_slice()));
        
        parse_interfaces(replies)
    }
        
    /// Sends out a search request for a specific UPnP interface on the specified from_addr. 
    ///
    /// This function returns one interface but does not check to see if more than 
    /// one device responded. If more then one device responded (which should not happen) 
    /// then the first response is what will be returned. 
    ///
    /// This is a blocking operation.
    pub fn find_uuid(from_addr: SocketAddr, uuid: &str) -> IoResult<Option<UPnPInterface>> {
        let uuid_st = String::from_str("uuid:{1}").replace("{1}", uuid);
        let request = String::from_str(GENERIC_SEARCH_REQUEST).replace("{1}", "5").replace("{2}", uuid_st.as_slice());
            
        let replies = try!(send_search(from_addr, 5500, request.as_slice()));
            
        match parse_interfaces(replies) {
            Ok(mut n) => Ok(n.remove(0)),
            Err(n) => Err(n)
        }
    }

    /// Check if the current UPnPInterface represents a root device on the network.
    pub fn is_root(&self) -> bool {
        match self {
            &Root(..) => true,
            _         => false
        }
    }

    /// Check if the current UPnPInterface represents a device on the network.
    ///
    /// Note that this function will return false if the type is a root device.
    pub fn is_device(&self) -> bool {
        match self {
            &Device(..) => true,
            _           => false
        }
    }
    
    /// Check if the current UPnPInterface represents a service on the network.
    pub fn is_service(&self) -> bool {
        match self {
            &Service(..) => true,
            _            => false
        }
    }
    
    /// Check if the current UPnPInterface represents an identifier (uuid) on the network.
    pub fn is_identifier(&self) -> bool {
        match self {
            &Identifier(..) => true,
            _               => false
        }
    }
    
    /// Get the device name, service specification, or uuid that this interface 
    /// has identified with.
    pub fn name<'a>(&'a self) -> &'a str {
        let (payload, (start, end)) = match self {
            &Root(ref n, seg, _, _, _, _)       => (n, seg),
            &Device(ref n, seg, _, _, _, _)     => (n, seg),
            &Service(ref n, seg, _, _, _, _)    => (n, seg),
            &Identifier(ref n, seg, _, _, _, _) => (n, seg)
        };
        
        payload.as_slice().slice(start, end)
    }

    /// Get the device version, service version, or no version (identifiers/root devices) 
    /// that this interface has identified with.
    pub fn version<'a>(&'a self) -> &'a str {
        let (payload, (start, end)) = match self {
            &Root(ref n, _, seg, _, _, _)       => (n, seg),
            &Device(ref n, _, seg, _, _, _)     => (n, seg),
            &Service(ref n, _, seg, _, _, _)    => (n, seg),
            &Identifier(ref n, _, seg, _, _, _) => (n, seg)
        };
        
        payload.as_slice().slice(start, end)
    }
    
    /// Get the location of the root device description page corresponding to 
    /// this UPnPInterface.
    pub fn location<'a>(&'a self) -> &'a str {
        let (payload, (start, end)) = match self {
            &Root(ref n, _, _, seg, _, _)       => (n, seg),
            &Device(ref n, _, _, seg, _, _)     => (n, seg),
            &Service(ref n, _, _, seg, _, _)    => (n, seg),
            &Identifier(ref n, _, _, seg, _, _) => (n, seg)
        };
        
        payload.as_slice().slice(start, end)
    }
    
    /// Get the search target.
    pub fn st<'a>(&'a self) -> &'a str {
        let (payload, (start, end)) = match self {
            &Root(ref n, _, _, _, seg, _)       => (n, seg),
            &Device(ref n, _, _, _, seg, _)     => (n, seg),
            &Service(ref n, _, _, _, seg, _)    => (n, seg),
            &Identifier(ref n, _, _, _, seg, _) => (n, seg)
        };
        
        payload.as_slice().slice(start, end)
    }
    
    /// Get the full Unique Service Name.
    pub fn usn<'a>(&'a self) -> &'a str {
        let (payload, (start, end)) = match self {
            &Root(ref n, _, _, _, _, seg)       => (n, seg),
            &Device(ref n, _, _, _, _, seg)     => (n, seg),
            &Service(ref n, _, _, _, _, seg)    => (n, seg),
            &Identifier(ref n, _, _, _, _, seg) => (n, seg)
        };
        
        payload.as_slice().slice(start, end)
    }

    /// Get the service description for this service.
    ///
    /// Note that if this is called on an object that is not service, an error
    /// will always be returned.
    ///
    /// This is a blocking operation.
    pub fn service_desc(&self) -> IoResult<ServiceDesc> {
        match self {
            &Service(..) => ServiceDesc::parse(self.location(), self.st()),
            _ => Err(util::get_error(InvalidInput, "UPnPInterface Is Not A Service"))
        }
    }
    
    /// Get the services that this device exposes.
    ///
    /// Note that the services returned are the immediate services that this device
    /// exposes. Any services that are nested within devices within this device will
    /// not be returned. An error will always be returned if this is called on an
    /// object that is of type service.
    ///
    /// This is a blocking operation.
    //pub fn service_descs(&self) -> IoResult<Vec<ServiceDesc>> {
        
    //}
}

/// Takes ownership of a vector of String objects that correspond to M-Search respones
/// and generates a vector of UPnPInterface objects. Each String object corresponds
/// to one UPnPInterface object.
fn parse_interfaces(payloads: Vec<String>) -> IoResult<Vec<UPnPInterface>> {
    let mut interfaces: Vec<UPnPInterface> = Vec::new();
    
    for i in payloads.into_iter() {
        let usn = try!(try!(USN_REGEX.captures(i.as_slice()).ok_or(
            util::get_error(InvalidInput, "USN Match Failed")
        )).pos(1).ok_or(util::get_error(InvalidInput, "USN Field Missing")));
        
        let st = try!(try!(ST_REGEX.captures(i.as_slice()).ok_or(
            util::get_error(InvalidInput, "ST Match Failed")
        )).pos(1).ok_or(util::get_error(InvalidInput, "ST Field Missing")));
        
        let location = try!(try!(LOCATION_REGEX.captures(i.as_slice()).ok_or(
            util::get_error(InvalidInput, "Location Match Failed")
        )).pos(1).ok_or(util::get_error(InvalidInput, "Location Field Missing")));
        
        if SERVICE_REGEX.is_match(i.as_slice()) {
            let (name, version): (StrPos, StrPos);

            { // Scope Borrow Of i In captures
                let captures = try!(SERVICE_REGEX.captures(i.as_slice()).ok_or(
                   util::get_error(InvalidInput, "Service Match Failed") 
                ));
                
                name = try!(captures.pos(1).ok_or(
                    util::get_error(InvalidInput, "Service Name Match Failed") 
                ));
                version = try!(captures.pos(2).ok_or(
                    util::get_error(InvalidInput, "Service Version Match Failed") 
                ));
            }

            interfaces.push(Service(i, name, version, location, st, usn));
        } else if DEVICE_REGEX.is_match(i.as_slice()) {
            let (name, version): (StrPos, StrPos);
            
            { // Scope Borrow Of i In captures
                let captures = try!(DEVICE_REGEX.captures(i.as_slice()).ok_or(
                   util::get_error(InvalidInput, "Device Match Failed") 
                ));
                
                name = try!(captures.pos(1).ok_or(
                    util::get_error(InvalidInput, "Device Name Match Failed") 
                ));
                version = try!(captures.pos(2).ok_or(
                    util::get_error(InvalidInput, "Device Version Match Failed") 
                ));
            }
        
            interfaces.push(Device(i, name, version, location, st, usn));
        } else if ROOT_REGEX.is_match(i.as_slice()) { // This HAS To Be Checked Before Identifier
            let name: StrPos;
            
            { // Scope Borrow Of i In captures
                let captures = try!(ROOT_REGEX.captures(i.as_slice()).ok_or(
                   util::get_error(InvalidInput, "Root Match Failed") 
                ));
                
                name = try!(captures.pos(1).ok_or(
                    util::get_error(InvalidInput, "Root Name Match Failed") 
                ));
            }
        
            interfaces.push(Root(i, name, (0, 0), location, st, usn));
        } else if IDENTIFIER_REGEX.is_match(i.as_slice()) {
            let name: StrPos;
            
            { // Scope Borrow Of i In captures
                let captures = try!(IDENTIFIER_REGEX.captures(i.as_slice()).ok_or(
                   util::get_error(InvalidInput, "Identifier Match Failed") 
                ));
                
                name = try!(captures.pos(1).ok_or(
                    util::get_error(InvalidInput, "Identifier Name Match Failed") 
                ));
            }
        
            interfaces.push(Identifier(i, name, (0, 0), location, st, usn));
        }  // Ignore Any Other (Invalid) ST Identifiers
    }
    
    Ok(interfaces)
}

/// Sends a UDP request to the standard UPnP multicast address used for device
/// discovery. The request is sent on the given from_addr using the supplied request.
///
/// Note that for larger networks you will want to increase the timeout value. However,
/// you will also want to scale the MX field accordingly so that replies are not dropped 
/// because of a flood of UDP packets either on the local interface or local router.
// TODO: Make this more robust by stuffing the text into the current String object
// until the \r\n\r\n is reached (this could span multiple receive requests) in case
// our static buffer is not big enough (rare case).
fn send_search(from_addr: SocketAddr, timeout: uint, request: &str) -> IoResult<Vec<String>> {
    let mut udp_sock = try!(UdpSocket::bind(from_addr));
    
    udp_sock.set_read_timeout(Some(timeout as u64));
    try!(udp_sock.send_to(request.as_bytes(), DISCOVERY_ADDR));
    
    let mut replies: Vec<String> = Vec::new();
    loop {
        let mut reply_buf = [0u8,..1000];
        
        match udp_sock.recv_from(reply_buf) {
            Ok(_) => {
                let end = try!(reply_buf.iter().position({ |&b|
                    b == 0u8
                }).ok_or(util::get_error(InvalidInput, "Search Reply Corrupt")));
                
                let payload: String = try!(reply_buf.slice_to(end).container_as_str().ok_or(
                    util::get_error(InvalidInput, "Search Reply Not A Valid String")
                )).into_string();
                
                replies.push(payload);
            },
            Err(_) => { break; }
        };
    }
    
    Ok(replies)
}