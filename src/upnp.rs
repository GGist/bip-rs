use util;
use regex::{Regex};
use std::old_path::{BytesContainer};
use std::borrow::{ToOwned};
use std::old_io::net::tcp::{TcpStream};
use std::old_io::{IoResult, InvalidInput};
use std::old_io::net::udp::{UdpSocket};
use std::old_io::net::ip::{SocketAddr, Ipv4Addr};

// http://upnp.org/sdcps-and-certification/standards/sdcps/
// http://www.upnp.org/specs/arch/UPnP-arch-DeviceArchitecture-v1.0-20080424.pdf

// {1} = Device Max Wait Period, {2} = Search Target
static DISCOVERY_ADDR: SocketAddr = SocketAddr{ ip: Ipv4Addr(239, 255, 255, 250), port: 1900 };
static GENERIC_SEARCH_REQUEST: &'static str = "M-SEARCH * HTTP/1.1\r
Host: 239.255.255.250:1900\r
Man: \"ssdp:discover\"\r
MX: {1}\r
ST: {2}\r\n\r\n";

// {1} = Path, {2} = IP Address:Port
static GENERIC_HTTP_REQUEST: &'static str = "GET {1} HTTP/1.1\r
Host: {2}\r
Connection: close\r\n\r\n";

// {1} = Control URL, {2} = IP Address:Port, {3} = Bytes In Payload,
// {4} = Search Target Of Service, {5} = Action Name, {6} = Action Parameters
static GENERIC_SOAP_REQUEST: &'static str = "POST {1} HTTP/1.1\r
Host: {2}\r
Connection: close\r
Content-Length: {3}\r
Content-Type: text/xml; charset=\"utf-8\"\r
SOAPAction: \"{4}#{5}\"\r
\r
<?xml version=\"1.0\"?>
<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\"
s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">
    <s:Body>
        <u:{5} xmlns:u=\"{4}\">{6}</u:{5}>
    </s:Body>
</s:Envelope>";
// Ignores Replace Identifiers And Escape Characters 
// (Including Implicit Newline Escape)
static BASE_CONTENT_LENGTH: usize = 216;

// UPnPIntf Classifiers
static ROOT_REGEX: Regex = regex!(r"upnp:(rootdevice)");
static DEVICE_REGEX: Regex = regex!(r":device:(.+?):(\d+)");
static SERVICE_REGEX: Regex = regex!(r":service:(.+?):(\d+)");
static IDENTIFIER_REGEX: Regex = regex!(r"uuid:([\w\d-]+)");

// Case Insensitive Matches For Search Response Header Fields
static ST_REGEX: Regex = regex!(r"(?i)ST: *([^\r\n]+)");
static USN_REGEX: Regex = regex!(r"(?i)USN: *([^\r\n]+)");
static LOCATION_REGEX: Regex = regex!(r"(?i)Location: *([^\r\n]+)");

/// A tuple consisting of start and end position.
pub type StrPos = (usize, usize);

/// Type used to represent a service description. Service actions can be viewed
/// as well as related in and out parameters and service state variables.
///
/// Note: As of now, this object makes no guarantee that messages it sends to UPnP
/// services that are not standard will be well formed; this is temporary as it allows
/// the code to remain clean until a proper solution arises. A list of standardized 
/// UPnP devices and their corresponding services can be found at 
/// http://upnp.org/sdcps-and-certification/standards/sdcps/.
pub struct ServiceDesc {
    location: SocketAddr,
    search_target: String,
    control_path: String,
    service_desc: String,
    actions: Vec<StrPos>,
    var_table: Option<StrPos>
}

impl ServiceDesc {
    /// Takes a location corresponding to the root device description page of a UPnPIntf
    /// as well as the st field of the interface which will always be unique to a service.
    ///
    /// This is a blocking operation.
    fn parse(location: &str, st: &str) -> IoResult<ServiceDesc> {
        // Setup Regex For Parsing Device Description Page
        let service_find = String::from_str("<service>\\s*?<serviceType>{1}</serviceType>(?:.|\n)+?</service>").replace("{1}", st);
        let service_regex = try!(Regex::new(service_find.as_slice()).or_else( |_|
            Err(util::simple_ioerror(InvalidInput, "Failed To Create Service Regex"))
        ));
        let scpd_find: Regex = regex!(r"<SCPDURL>(.+?)</SCPDURL>");
        let control_find: Regex = regex!(r"<controlURL>(.+?)</controlURL>");
        
        // Setup Regex For Parsing Service Description Page
        let action_regex: Regex = regex!(r" *<action>(?:.|\n)+?</action>");
        let state_vars_regex: Regex = regex!(r"<serviceStateTable>(?:.|\n)+?</serviceStateTable>");
        
        // Setup Data To Query Device Description Page
        let path = try!(util::extract_path(location));
        let sock_addr = try!(util::lookup_url(location));
        let request = String::from_str(GENERIC_HTTP_REQUEST).replace("{1}", path)
            .replace("{2}", sock_addr.to_string().as_slice());

        // Query Device Description Page
        let mut tcp_sock = try!(TcpStream::connect(sock_addr));

        try!(tcp_sock.write_str(request.as_slice()));
        let response = try!(tcp_sock.read_to_string());
        
        let (tag_begin, tag_end) = try!(service_regex.find(response.as_slice()).ok_or(
            util::simple_ioerror(InvalidInput, "Could Not Find Service Tag")
        ));

        // Pull Service Description Path And Control Path
        let service_tag = &response[tag_begin..tag_end];
        
        let scpd_path = try!(try!(scpd_find.captures(service_tag).ok_or(
            util::simple_ioerror(InvalidInput, "Could Not Capture SCPD Path")
        )).at(1).ok_or(
           util::simple_ioerror(InvalidInput, "Could Not Capture SCPD Path") 
        ));
        let control_path = try!(try!(control_find.captures(service_tag).ok_or(
            util::simple_ioerror(InvalidInput, "Could Not Capture Control Path")
        )).at(1).ok_or(
            util::simple_ioerror(InvalidInput, "Could Not Capture Control Path")
        ));

        // Query Service Description
        tcp_sock = try!(TcpStream::connect(sock_addr));
        let request = request.replace(path.as_slice(), scpd_path);

        try!(tcp_sock.write_str(request.as_slice()));
        // We Will Be Returning response Slices To Users, So We Format The Response
        let response = try!(tcp_sock.read_to_string()).replace("\t", " ");

        // Pull Out Actions And State Variable Table Locations
        let actions = action_regex.find_iter(response.as_slice()).collect::<Vec<StrPos>>();
        let var_table = state_vars_regex.find(response.as_slice());
        
        Ok(ServiceDesc{ location: sock_addr, search_target: st.to_string(), 
            control_path: control_path.to_string(), service_desc: response, 
            actions: actions, var_table: var_table })
    }
    
    /// Returns a list of string slices corresponding to each action that can be
    /// sent to this service. The slice includes the action name, and any in or out
    /// parameters expected/provided as well as their related state variables. The 
    /// "relatedStateVariable" corresponds to an entry in the "serviceStateTable".
    ///
    /// The response is left in the XML format that it was sent from the UPnPIntf
    /// in. We leave it in this format so that we don't spend time allocating memory and
    /// parsing responses.
    pub fn actions<'a>(&'a self) -> Vec<&'a str> {
        let mut actions_list: Vec<&'a str> = Vec::new();
        
        for &(start, end) in self.actions.iter() {
            actions_list.push(&self.service_desc[start..end]);
        }
        
        actions_list
    }
    
    /// Optionally returns a string slice corresponding to the "serviceStateTable"
    /// of the service description. This table explains the data types referenced
    /// in the "relatedStateVariable" field for each in or out parameters for each
    /// action.
    ///
    /// The response is left in the XML format that it was sent from the UPnPIntf
    /// in. We leave it in this format so that we don't spend time allocating memory and
    /// parsing responses.
    pub fn state_variables<'a>(&'a self) -> Option<&'a str> {
        match self.var_table {
            Some((start, end)) => Some(&self.service_desc[start..end]),
            None               => None
        }
    }
    
    /// Takes an action string representing the procedure to call as well as an
    /// array slice of tuples in the form (arg_name, arg_value). An empty array 
    /// slice indicates that no parameters are expected for the specified action.
    ///
    /// This operation will not check to make sure that valid actions or parameters
    /// have been passed in, so any error handling may happen on the service end.
    ///
    /// This is a blocking operation.
    pub fn send_action(&self, action: &str, params: &[(&str, &str)]) -> IoResult<String> {
        let response_find = String::from_str("<\\w*:?{1}Response(?:.|\n)+?</\\w*:?{1}Response>").replace("{1}", action);
        let response_regex = try!(Regex::new(response_find.as_slice()).or_else( |_|
            Err(util::simple_ioerror(InvalidInput, "Failed To Create Response Regex"))
        ));
    
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

        // Send Request
        let mut tcp_sock = try!(TcpStream::connect(self.location));
        
        try!(tcp_sock.write_str(request.as_slice()));
        let response = try!(tcp_sock.read_to_string());
        
        // Service Should Be Using HTTP Codes To Signify Errors
        if !response.as_slice().contains("200 OK") {
            return Err(util::simple_ioerror(InvalidInput, "Service Returned An Error"));
        }
        
        let (a, b) = try!(response_regex.find(response.as_slice()).ok_or(
            util::simple_ioerror(InvalidInput, "Unexpected Procedure Return Value")
        ));
        Ok(response[a..b].to_string())
    }
}

/// Type used to represent different interfaces available on a network as defined 
/// by the UPnP specification. This type can be used to get very general information
/// about an interface but also to get access to the services exposed by the interface.
pub enum UPnPIntf {
    #[doc(hidden)]
    Root(String, StrPos, StrPos, StrPos, StrPos, StrPos),
    #[doc(hidden)]
    Device(String, StrPos, StrPos, StrPos, StrPos, StrPos),
    #[doc(hidden)]
    Service(String, StrPos, StrPos, StrPos, StrPos, StrPos),
    #[doc(hidden)]
    Identifier(String, StrPos, StrPos, StrPos, StrPos, StrPos)
}

impl UPnPIntf {
    /// Sends out a search request for all UPnP interfaces on the specified from_addr.
    ///
    /// This is a blocking operation.
    pub fn find_all(from_addr: SocketAddr) -> IoResult<Vec<UPnPIntf>> {
        let request = String::from_str(GENERIC_SEARCH_REQUEST).replace("{1}", "5").replace("{2}", "ssdp:all");
        let replies = try!(send_search(from_addr, 5500, request.as_slice()));
        
        parse_interfaces(replies)
    }
    
    /// Sends out a search request for particular UPnP services on the specified from_addr. 
    ///
    /// This method sends out a search request asking that only services with the
    /// specified name and version respond. This method does not check to make
    /// sure that only services with the specified name and version responded.
    /// Additonally, any service interfaces that are not standard UPnP services
    /// http://upnp.org/sdcps-and-certification/standards/sdcps/ will not be found
    /// and should be discovered by using find_all and filtering the results.
    ///
    /// This is a blocking operation.
    pub fn find_services(from_addr: SocketAddr, name: &str, version: &str) -> IoResult<Vec<UPnPIntf>> {
        let service_st = String::from_str("urn:schemas-upnp-org:service:{1}:{2}").replace("{1}", name).replace("{2}", version);
        let request = String::from_str(GENERIC_SEARCH_REQUEST).replace("{1}", "2").replace("{2}", service_st.as_slice());
        
        let replies = try!(send_search(from_addr, 2200, request.as_slice()));
        
        parse_interfaces(replies)
    }
            
    /// Sends out a search request for particular UPnP devices on the specified from_addr. 
    ///
    /// This method sends out a search request asking that only devices with the
    /// specified name and version respond. This method does not check to make
    /// sure that only devices with the specified name and version responded.
    /// Additionally, any device interface that are not standard UPnP devices
    /// (http://upnp.org/sdcps-and-certification/standards/sdcps/) will not be found
    /// and should be discovered by using find_all and filtering the results.
    ///
    /// This is a blocking operation.
    pub fn find_devices(from_addr: SocketAddr, name: &str, version: &str) -> IoResult<Vec<UPnPIntf>> {
        let device_st = String::from_str("urn:schemas-upnp-org:device:{1}:{2}").replace("{1}", name).replace("{2}", version);
        let request = GENERIC_SEARCH_REQUEST.replace("{1}", "2").replace("{2}", device_st.as_slice());
        
        let replies = try!(send_search(from_addr, 2200, request.as_slice()));
        
        parse_interfaces(replies)
    }
        
    /// Sends out a search request for a unique UPnP interface on the specified from_addr. 
    ///
    /// This method sends out a search request asking that only the device with the
    /// specified uuid respond. This method does not check the the device that responded
    /// has the right uuid. If more than one device responds (which should not happen
    /// in any correct implementation) then the first device to respond will be returned.
    ///
    /// This is a blocking operation.
    pub fn find_uuid(from_addr: SocketAddr, uuid: &str) -> IoResult<Option<UPnPIntf>> {
        let uuid_st = String::from_str("uuid:{1}").replace("{1}", uuid);
        let request = String::from_str(GENERIC_SEARCH_REQUEST).replace("{1}", "5").replace("{2}", uuid_st.as_slice());
            
        let replies = try!(send_search(from_addr, 5500, request.as_slice()));
            
        match parse_interfaces(replies) {
            Ok(mut n) => Ok(Some(n.remove(0))),
            Err(n) => Err(n)
        }
    }

    /// Check if the current UPnPIntf represents a root device on the network.
    pub fn is_root(&self) -> bool {
        match self {
            &UPnPIntf::Root(..) => true,
            _         => false
        }
    }

    /// Check if the current UPnPIntf represents a device on the network.
    ///
    /// Note: This function will return false in the case of a root (even though
    /// it is also technically a device).
    pub fn is_device(&self) -> bool {
        match self {
            &UPnPIntf::Device(..) => true,
            _           => false
        }
    }
    
    /// Check if the current UPnPIntf represents a service on the network.
    pub fn is_service(&self) -> bool {
        match self {
            &UPnPIntf::Service(..) => true,
            _            => false
        }
    }
    
    /// Check if the current UPnPIntf represents an identifier (uuid) on the network.
    ///
    /// Note: This function will return false in the case of a root or device even 
    /// though all uuid's refer to a specific device (or root in some cases).
    pub fn is_identifier(&self) -> bool {
        match self {
            &UPnPIntf::Identifier(..) => true,
            _               => false
        }
    }
    
    /// Get the device name, service specification, or uuid that this interface 
    /// has identified itself with.
    pub fn name<'a>(&'a self) -> &'a str {
        let (payload, (start, end)) = match self {
            &UPnPIntf::Root(ref n, seg, _, _, _, _)       => (n, seg),
            &UPnPIntf::Device(ref n, seg, _, _, _, _)     => (n, seg),
            &UPnPIntf::Service(ref n, seg, _, _, _, _)    => (n, seg),
            &UPnPIntf::Identifier(ref n, seg, _, _, _, _) => (n, seg)
        };
        
        &payload[start..end]
    }
    
    /// Get the device version, service version, or no version (in the case of an 
    /// identifier or root) that this interface has identified itself with.
    pub fn version<'a>(&'a self) -> &'a str {
        let (payload, (start, end)) = match self {
            &UPnPIntf::Root(ref n, _, seg, _, _, _)       => (n, seg),
            &UPnPIntf::Device(ref n, _, seg, _, _, _)     => (n, seg),
            &UPnPIntf::Service(ref n, _, seg, _, _, _)    => (n, seg),
            &UPnPIntf::Identifier(ref n, _, seg, _, _, _) => (n, seg)
        };
        
        &payload[start..end]
    }
    
    /// Get the location (url) of the root device description web page corresponding
    /// to this particular UPnPIntf.
    pub fn location<'a>(&'a self) -> &'a str {
        let (payload, (start, end)) = match self {
            &UPnPIntf::Root(ref n, _, _, seg, _, _)       => (n, seg),
            &UPnPIntf::Device(ref n, _, _, seg, _, _)     => (n, seg),
            &UPnPIntf::Service(ref n, _, _, seg, _, _)    => (n, seg),
            &UPnPIntf::Identifier(ref n, _, _, seg, _, _) => (n, seg)
        };
        
        &payload[start..end]
    }
    
    /// Get the search target of this UPnPIntf.
    pub fn st<'a>(&'a self) -> &'a str {
        let (payload, (start, end)) = match self {
            &UPnPIntf::Root(ref n, _, _, _, seg, _)       => (n, seg),
            &UPnPIntf::Device(ref n, _, _, _, seg, _)     => (n, seg),
            &UPnPIntf::Service(ref n, _, _, _, seg, _)    => (n, seg),
            &UPnPIntf::Identifier(ref n, _, _, _, seg, _) => (n, seg)
        };
        
        &payload[start..end]
    }
    
    /// Get the full Unique Service Name of this UPnPIntf.
    pub fn usn<'a>(&'a self) -> &'a str {
        let (payload, (start, end)) = match self {
            &UPnPIntf::Root(ref n, _, _, _, _, seg)       => (n, seg),
            &UPnPIntf::Device(ref n, _, _, _, _, seg)     => (n, seg),
            &UPnPIntf::Service(ref n, _, _, _, _, seg)    => (n, seg),
            &UPnPIntf::Identifier(ref n, _, _, _, _, seg) => (n, seg)
        };
        
        &payload[start..end]
    }

    /// Get a service description object for this service.
    ///
    /// Note: This will always fail if called on a non-service UPnPIntf.
    ///
    /// This is a blocking operation.
    pub fn service_desc(&self) -> IoResult<ServiceDesc> {
        match self {
            &UPnPIntf::Service(..) => ServiceDesc::parse(self.location(), self.st()),
            _ => Err(util::simple_ioerror(InvalidInput, "UPnPIntf Is Not A Service"))
        }
    }
}

/// Takes ownership of a vector of String objects that correspond to M-Search respones
/// and generates a vector of UPnPIntf objects. Each String object corresponds
/// to one UPnPIntf object.
fn parse_interfaces(payloads: Vec<String>) -> IoResult<Vec<UPnPIntf>> {
    let mut interfaces: Vec<UPnPIntf> = Vec::new();
    
    for i in payloads.into_iter() {
        let usn = try!(try!(USN_REGEX.captures(i.as_slice()).ok_or(
            util::simple_ioerror(InvalidInput, "USN Match Failed")
        )).pos(1).ok_or(util::simple_ioerror(InvalidInput, "USN Field Missing")));
        
        let st = try!(try!(ST_REGEX.captures(i.as_slice()).ok_or(
            util::simple_ioerror(InvalidInput, "ST Match Failed")
        )).pos(1).ok_or(util::simple_ioerror(InvalidInput, "ST Field Missing")));
        
        let location = try!(try!(LOCATION_REGEX.captures(i.as_slice()).ok_or(
            util::simple_ioerror(InvalidInput, "Location Match Failed")
        )).pos(1).ok_or(util::simple_ioerror(InvalidInput, "Location Field Missing")));
        
        if SERVICE_REGEX.is_match(i.as_slice()) {
            let (name, version): (StrPos, StrPos);

            { // Scope Borrow Of i In captures
                let captures = try!(SERVICE_REGEX.captures(i.as_slice()).ok_or(
                   util::simple_ioerror(InvalidInput, "Service Match Failed") 
                ));
                
                name = try!(captures.pos(1).ok_or(
                    util::simple_ioerror(InvalidInput, "Service Name Match Failed") 
                ));
                version = try!(captures.pos(2).ok_or(
                    util::simple_ioerror(InvalidInput, "Service Version Match Failed") 
                ));
            }

            interfaces.push(UPnPIntf::Service(i, name, version, location, st, usn));
        } else if DEVICE_REGEX.is_match(i.as_slice()) {
            let (name, version): (StrPos, StrPos);
            
            { // Scope Borrow Of i In captures
                let captures = try!(DEVICE_REGEX.captures(i.as_slice()).ok_or(
                   util::simple_ioerror(InvalidInput, "Device Match Failed") 
                ));
                
                name = try!(captures.pos(1).ok_or(
                    util::simple_ioerror(InvalidInput, "Device Name Match Failed") 
                ));
                version = try!(captures.pos(2).ok_or(
                    util::simple_ioerror(InvalidInput, "Device Version Match Failed") 
                ));
            }
        
            interfaces.push(UPnPIntf::Device(i, name, version, location, st, usn));
        } else if ROOT_REGEX.is_match(i.as_slice()) { // This HAS To Be Checked Before Identifier
            let name: StrPos;
            
            { // Scope Borrow Of i In captures
                let captures = try!(ROOT_REGEX.captures(i.as_slice()).ok_or(
                   util::simple_ioerror(InvalidInput, "Root Match Failed") 
                ));
                
                name = try!(captures.pos(1).ok_or(
                    util::simple_ioerror(InvalidInput, "Root Name Match Failed") 
                ));
            }
        
            interfaces.push(UPnPIntf::Root(i, name, (0, 0), location, st, usn));
        } else if IDENTIFIER_REGEX.is_match(i.as_slice()) {
            let name: StrPos;
            
            { // Scope Borrow Of i In captures
                let captures = try!(IDENTIFIER_REGEX.captures(i.as_slice()).ok_or(
                   util::simple_ioerror(InvalidInput, "Identifier Match Failed") 
                ));
                
                name = try!(captures.pos(1).ok_or(
                    util::simple_ioerror(InvalidInput, "Identifier Name Match Failed") 
                ));
            }
        
            interfaces.push(UPnPIntf::Identifier(i, name, (0, 0), location, st, usn));
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
fn send_search(from_addr: SocketAddr, timeout: usize, request: &str) -> IoResult<Vec<String>> {
    let mut udp_sock = try!(UdpSocket::bind(from_addr));
    
    udp_sock.set_read_timeout(Some(timeout as u64));
    try!(udp_sock.send_to(request.as_bytes(), DISCOVERY_ADDR));
    
    let mut replies: Vec<String> = Vec::new();
    loop {
        let mut reply_buf = [0u8; 1000];
        
        match udp_sock.recv_from(&mut reply_buf) {
            Ok(_) => {
                let end = try!(reply_buf.iter().position({ |b|
                    *b == 0u8
                }).ok_or(util::simple_ioerror(InvalidInput, "Search Reply Corrupt")));
                
                let payload: String = try!(reply_buf[..end].container_as_str().ok_or(
                    util::simple_ioerror(InvalidInput, "Search Reply Not A Valid String")
                )).to_owned();
                
                replies.push(payload);
            },
            Err(_) => { break; }
        };
    }
    
    Ok(replies)
}