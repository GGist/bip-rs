use util;
use bencode::{BenVal, Dict, Bytes, List, Int};
use std::path::BytesContainer;
use std::collections::{HashMap};
use std::io::{InvalidInput, IoResult};

pub struct Torrent<'a> {
    pub announce: &'a str,
    pub announce_list: Option<Vec<&'a str>>,
    pub comment: Option<&'a str>,
    pub created_by: Option<&'a str>,
    pub creation_date: Option<&'a str>,
    
    info: FileType<'a>
}

impl<'a> Torrent<'a> {
    pub fn new<'a>(ben_torr: &'a BenVal) -> IoResult<Torrent<'a>> {
        let torr_dict = try!(ben_torr.dict().ok_or(
            util::get_error(InvalidInput, "ben_torr Is Not A Dict Value")
        ));
        
        let announce = match torr_dict.find_equiv(&"announce") {
            Some(&Bytes(ref n)) => try!(n.container_as_str().ok_or(
                util::get_error(InvalidInput, "announce Not Valid UTF-8 In torr_dict")
            )),
            _ => { return Err(util::get_error(InvalidInput, "announce Key Not In ben_torr")) }
        };
        
        // The Announce List Is Going To Be A 2D List, Each Inner List Is A 
        // "Tier" [[backup1, backup2], [backup3], [backup4, backup5]]
        let mut announce_list: Option<Vec<&'a str>> = Some(Vec::new());
        match torr_dict.find_equiv(&"announce-list") {
            Some(&List(ref n)) => {
                for i in n.iter() {
                    let list_tier = try!(i.list().ok_or(
                        util::get_error(InvalidInput, "list_tier Is Not A List Value")
                    ));

                    for j in list_tier.iter() {
                        let tier_str = try!(j.str().ok_or(
                            util::get_error(InvalidInput, "tier_str Is Not A String Value")
                        ));

                        // announce_list Will ALWAYS Be Some At This Point
                        announce_list.as_mut().unwrap().push(tier_str);
                    }
                }
            },
            _ => { announce_list = None }
        };
        
        // In Case Torrent File Had An Empty Announce-List
        if (announce_list.is_some() && announce_list.as_ref().unwrap().len() == 0) {
            announce_list = None;
        }
        // Spec Says We Should Randomize Backup Trackers Within Each Tier
        
        let comment = match torr_dict.find_equiv(&"comment") {
            Some(&Bytes(ref n)) => Some(try!(n.container_as_str().ok_or(
                util::get_error(InvalidInput, "comment Not Valid UTF-8 In torr_dict")
            ))),
            _ => None
        };
        
        let created_by = match torr_dict.find_equiv(&"created by") {
            Some(&Bytes(ref n)) => Some(try!(n.container_as_str().ok_or(
                util::get_error(InvalidInput, "created by Not Valid UTF-8 In torr_dict")
            ))),
            _ => None
        };
        
        // Torrent Files May Incorrectly Expose This As A BenVal::Int
        let creation_date = match torr_dict.find_equiv(&"creation date") {
            Some(&Bytes(ref n)) => Some(try!(n.container_as_str().ok_or(
                util::get_error(InvalidInput, "creation date Not Valid UTF-8 In torr_dict")
            ))),
            _ => None
        };
        
        let info = match torr_dict.find_equiv(&"info") {
            Some(n) => try!(FileType::new(n)),
            _ => { return Err(util::get_error(InvalidInput, "info Key Not In torr_dict")) }
        };
        
        Ok(Torrent{ announce: announce, announce_list: announce_list, 
            comment: comment, created_by: created_by, 
            creation_date: creation_date, info: info }
        )
    }
    
    pub fn is_single_file(&self) -> bool {
        match self.info {
            SingleFile(..) => true,
            MultiFile(..) => false
        }
    }
    
    pub fn is_multi_file(&self) -> bool {
        match self.info {
            SingleFile(..) => false,
            MultiFile(..) => true
        }
    }
    
    pub fn num_files(&self) -> uint {
        match self.info {
            SingleFile(..) => 1,
            MultiFile(ref files, _, _, _) => files.len()
        }
    }
    
    pub fn get_file(&'a self, index: uint) -> IoResult<&'a File> {
        if (index > self.num_files()) {
            return Err(util::get_error(InvalidInput, "File index Out Of Range"))
        }
    
        match self.info {
            SingleFile(ref file, _, _, _) => Ok(file),
            MultiFile(ref files, _, _, _) => Ok(&files[index])
        }
    }
}

pub struct File<'a> {
    pub length: uint,
    pub md5sum: Option<&'a str>,
    pub path: Option<Vec<&'a str>>
}

// File(s), Name, Piece Length, Pieces
enum FileType<'a> {
    SingleFile(File<'a>, &'a str, uint, &'a [u8]),
    MultiFile(Vec<File<'a>>, &'a str, uint, &'a [u8])
}

impl<'a> FileType<'a> {
    fn new<'a>(ben_info: &'a BenVal) -> IoResult<FileType<'a>> {
        let info_dict = try!(ben_info.dict().ok_or(
            util::get_error(InvalidInput, "ben_info Is Not BenVal::Dict")
        ));
    
        if (info_dict.contains_key_equiv(&"files")) {
            FileType::multi_file(info_dict)
        } else {
            FileType::single_file(info_dict)
        }
    }
    
    fn single_file(info_dict: &'a HashMap<String, BenVal>) -> IoResult<FileType<'a>> {
        let length = match info_dict.find_equiv(&"length") {
            Some(&Int(n)) => n as uint,
            _ => { return Err(util::get_error(InvalidInput, "length Key Not In info_dict")) }
        };
        
        let md5sum = match info_dict.find_equiv(&"md5sum") {
            Some(&Bytes(ref n)) => Some(try!(n.container_as_str().ok_or(
                    util::get_error(InvalidInput, "md5sum Not Valid UTF-8 In info_dict")
            ))),
            _ => None
        };
        
        Ok(SingleFile(
            File{ length: length, md5sum: md5sum, path: None },
            try!(FileType::info_name(info_dict)),
            try!(FileType::info_piece_length(info_dict)), 
            try!(FileType::info_pieces(info_dict)))
        )
    }
    
    fn multi_file(info_dict: &'a HashMap<String, BenVal>) -> IoResult<FileType<'a>> {
        let files = match info_dict.find_equiv(&"files") {
            Some(&List(ref n)) => n,
            _ => { return Err(util::get_error(InvalidInput, "files Key Not In info_dict")) }
        };
        
        let mut files_list: Vec<File> = Vec::with_capacity(files.len());
        for i in files.iter() {
            let file = try!(i.dict().ok_or(
                util::get_error(InvalidInput, "files Is Not A BenVal::Dict")
            ));
            
            let length = match file.find_equiv(&"length") {
                Some(&Int(n)) => n as uint,
                _ => { return Err(util::get_error(InvalidInput, "length Key Not In file")) }
            };
            
            let md5sum = match file.find_equiv(&"md5sum") {
                Some(&Bytes(ref n)) => Some(try!(n.container_as_str().ok_or(
                        util::get_error(InvalidInput, "md5sum Not Valid UTF-8 In file")
                ))),
                _ => None
            };
            
            let mut path: Vec<&'a str> = Vec::new();
            let path_list = match file.find_equiv(&"path") {
                Some(&List(ref n)) => n,
                _ => { return Err(util::get_error(InvalidInput, "path Key Not In file")) }
            };
            
            for j in path_list.iter() {
                let curr_path = try!(j.str().ok_or(
                    util::get_error(InvalidInput, "path Entry In file Not A String Value")
                ));
                
                path.push(curr_path);
            }
            
            files_list.push(File{ length: length, md5sum: md5sum, 
                path: Some(path) }
            );
        }
        
        Ok(MultiFile(
            files_list,
            try!(FileType::info_name(info_dict)),
            try!(FileType::info_piece_length(info_dict)), 
            try!(FileType::info_pieces(info_dict)))
        )
    }
    
    fn info_name<'a>(info_dict: &'a HashMap<String, BenVal>) -> IoResult<&'a str> {
        match info_dict.find_equiv(&"name") {
            Some(&Bytes(ref n)) => Ok(try!(n.container_as_str().ok_or(
                    util::get_error(InvalidInput, "name Key In info_dict Not A String Value")
            ))),
            _ => Err(util::get_error(InvalidInput, "name Key Not Found In info_dict"))
        }
    }
    
    fn info_piece_length<'a>(info_dict: &'a HashMap<String, BenVal>) -> IoResult<uint> {
        match info_dict.find_equiv(&"piece length") {
            Some(&Int(n)) => Ok(n as uint),
            _ => Err(util::get_error(InvalidInput, "piece length Key Not In info_dict"))
        }
    }
    
    fn info_pieces<'a>(info_dict: &'a HashMap<String, BenVal>) -> IoResult<&'a [u8]> {
        match info_dict.find_equiv(&"pieces") {
            Some(&Bytes(ref n)) => Ok(n.as_slice()),
            _ => Err(util::get_error(InvalidInput, "pieces Key Not In info_dict"))
        }
    }
}