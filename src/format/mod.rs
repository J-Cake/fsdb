pub mod parse;
pub mod serialise;

use std::io::Error;
use std::io::Cursor;
use std::io::Result;
use std::io::Read;

pub use parse::*;
use serde::{Serialize, de::DeserializeOwned};
pub use serialise::*;

#[inline]
pub fn round(x: u64, n: u64) -> u64 {
    x + (n - x % n)
}

#[derive(Copy, Clone, Debug)]
pub struct Array {
    pub length: u64,
    pub offset: u64,
}

impl PartialEq for Array {
    fn eq(&self, other: &Self) -> bool {
        self.offset == other.offset
    }
}

impl Eq for Array {}

impl PartialOrd for Array {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Ord for Array {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.offset.cmp(&other.offset)
    }
}

pub fn blank<Meta>() -> Result<crate::Database<Cursor<Vec<u8>>>> where Meta: Serialize + DeserializeOwned + Clone + Default {
    let metadata = Meta::default();
    let meta = ron::ser::to_string(&metadata)
        .map_err(Error::other)?
        .into_bytes();
        
    let data_offset = crate::round((0x50u64 + meta.len() as u64)
        .max(0x80), 0x10);
    
    let mut header: Cursor<Vec<u8>> = Cursor::new(vec![
        &b"FSDB"[..], &u32::to_le_bytes(0x01)[..], &u64::to_le_bytes(0x00)[..],
        &u64::to_le_bytes(0x01)[..], &u64::to_le_bytes(data_offset)[..], // INode Table
        &u64::to_le_bytes(0x02)[..], &u64::to_le_bytes(data_offset + 0x100)[..], // String Table
        &u64::to_le_bytes(0x01)[..], &u64::to_le_bytes(data_offset + 0x200)[..], // History Table
        &u64::to_le_bytes(meta.len() as u64)[..], &u64::to_le_bytes(0x50)[..],
        &meta[..]
    ]
        .into_iter()
        .flatten()
        .cloned()
        .collect()
    );
    
    let mut raw_header = vec![0u8; 0x50];
    header.read_exact(&mut raw_header)?;
    
    todo!()
    
//     Ok(crate::Database {
//         raw_header: raw_header.to_vec(),
//         backing: Rc::new(RefCell::new(header)),
//         inode_table_range: crate::Array { length: 1, offset: data_offset },
//         string_table_range: crate::Array { length: 2, offset: data_offset + 0x100 },
//         history_table_range: crate::Array { length: 1, offset: data_offset + 0x200 },
//         metadata_range: crate::Array { length: meta.len() as u64, offset: 0x50 },
//         
//         inode_table_size: 0x20, // this value is found by ... just measuring... not elegant, but simpler/faster than computing it.
//         string_table_size: 0x12, // this value is also looked up.
//         history_table_size: 0, // history tab not defined yet
//         
//         borrowed_slices: Arc::new(Mutex::new(vec![])),
//         
//         inode_table: vec![("/".to_string(), crate::PageDescriptor {
//             name: "/".to_string(),
//             access_control_list: vec![crate::Access::ReadWriteExecute("*".to_string())],
//             modified: SystemTime::now(),
//             created: SystemTime::now(),
//             inodes: vec![]
//         })]
//             .into_iter()
//             .collect(),
//         // Upon serialisation, the missing strings will be inserted into the string table, but for completeness' sake, include them here.
//         string_table: RefCell::new(vec!["/".to_string(), "*".to_string()]),
//         meta: metadata
//     })
} 
