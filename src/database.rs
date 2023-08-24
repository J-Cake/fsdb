use std::collections::HashMap;
use std::io::BufWriter;
use std::io::Cursor;
use std::io::Error;
use std::io::Read;
use std::io::Write;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Result;
use std::ops::DerefMut;
use std::time::SystemTime;
use std::cell::RefCell;
use std::cell::RefMut;
use std::cell::Ref;
use std::rc::Rc;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::PageDescriptor;

/// A proxy which provides a reading and writing interface to the database's buffer.
#[derive(Clone)]
pub(crate) struct DBAgent<Buffer, Allocate> 
where 
    Buffer: Read + Write + Seek,
    Allocate: FnMut(u64) -> Result<Vec<(u64, u64)>>
{
    buffer: Rc<RefCell<Buffer>>,
    allocate: Allocate
}

impl<Buffer, Allocate> DBAgent<Buffer, Allocate> 
where 
    Buffer: Read + Write + Seek,
    Allocate: FnMut(u64) -> Result<Vec<(u64, u64)>>
{
    pub fn try_borrow_mut(&self) -> Result<RefMut<Buffer>> {
        self.buffer.try_borrow_mut()
            .map_err(Error::other)
    }
    
    pub fn try_transparent_borrow_mut(&mut self) -> Result<RefMut<Buffer>> {
        self.try_borrow_mut()
    }
    
    pub fn try_borrow(&self) -> Result<Ref<Buffer>> {
        self.buffer.try_borrow()
            .map_err(Error::other)
    }
    
    pub fn allocate_chunks(&mut self, min_size: u64) -> Result<Vec<(u64, u64)>> {
        (self.allocate)(min_size)
    }
}

/// Contains information about the database, providing a clean interface to accessing it
pub struct Database<Buffer, Metadata> where Buffer: Read + Write + Seek, Metadata: Serialize + DeserializeOwned + Clone {
    /// The underlying data source. As long as it supports Read, Write and Seek operations, this can be anything.
    pub(crate) backing: Rc<RefCell<Buffer>>,
    /// Offset of the inode table, and the number of elements in it.
    pub(crate) inode_table_range: (u64, u64),
    /// Offset of the string table, and the number of elements in it
    pub(crate) string_table_range: (u64, u64),
    /// Offset of the history table, and the number of elements in it
    pub(crate) history_table_range: (u64, u64),
    
    inode_table: HashMap<String, PageDescriptor>,
    string_table: RefCell<Vec<String>>,
    
    raw_header: Vec<u8>,
    pub meta: Metadata
}

impl<Buffer, Metadata> Database<Buffer, Metadata> where Buffer: Read + Write + Seek, Metadata: Serialize + DeserializeOwned + Clone {
    pub fn open(mut backing: Buffer) -> Result<Self> {
        backing.seek(std::io::SeekFrom::Start(0))?;
        
        let mut buf = vec![0u8; 4 + 4 + 4 + 4 + (4 * (2 * 8))];
        backing.read_exact(&mut buf)?;
        if &buf[0..4] != b"FSDB" { return Err(Error::other("Invalid Magic Number")); }
        if buf[4..8] != [0x01, 0, 0, 0] { return Err(Error::other("Unrecognised version")); }
        
        let inode_table_range = (u64::from_le_bytes(buf[16..24]
                .try_into()
                .map_err(Error::other)?), u64::from_le_bytes(buf[24..32]
                .try_into()
                .map_err(Error::other)?));
                
        let string_table_range = (u64::from_le_bytes(buf[32..40]
                .try_into()
                .map_err(Error::other)?), u64::from_le_bytes(buf[40..48]
                .try_into()
                .map_err(Error::other)?));
                
        let history_table_range = (u64::from_le_bytes(buf[48..56]
                .try_into()
                .map_err(Error::other)?), u64::from_le_bytes(buf[56..64]
                .try_into()
                .map_err(Error::other)?));
        
        let backing = Rc::new(RefCell::new(backing));
        
        let x = Ok(Self {
            inode_table: Self::parse_inode_table(Rc::clone(&backing)
                .try_borrow_mut()
                .map_err(Error::other)?, inode_table_range.0, inode_table_range.1)?,
            string_table: RefCell::new(Self::parse_string_table(Rc::clone(&backing)
                .try_borrow_mut()
                .map_err(Error::other)?, string_table_range.0, string_table_range.1)?),
            
            inode_table_range,
            string_table_range,
            history_table_range,
            raw_header: buf.clone(),
            meta: {
                let meta = (u64::from_le_bytes(buf[64..72]
                    .try_into()
                    .map_err(Error::other)?), u64::from_le_bytes(buf[72..80]
                    .try_into()
                    .map_err(Error::other)?));
                    
                let mut s = vec![0u8; meta.1 as usize];
                backing
                    .try_borrow_mut()
                    .map_err(Error::other)?
                    .read_exact(&mut s)?;
                
                ron::de::from_bytes::<Metadata>(&s)
                    .map_err(Error::other)?
                    .clone()
            },
            
            backing: Rc::clone(&backing),
        });
        
        return x;
    }
    
    pub fn blank<Meta>() -> Result<Database<Cursor<Vec<u8>>, Meta>> where Meta: Serialize + DeserializeOwned + Clone + Default {
        let metadata = Meta::default();
        let meta = ron::ser::to_string(&metadata)
            .map_err(Error::other)?
            .into_bytes();
            
        let data_offset = (0x50u64 + meta.len() as u64)
            .max(0x80);
        let data_offset = data_offset + (0x10 - data_offset % 0x10);
        
        let mut header: Cursor<Vec<u8>> = Cursor::new(vec![
            &b"FSDB"[..], &u32::to_le_bytes(0x01)[..], &u64::to_le_bytes(0x00)[..],
            &u64::to_le_bytes(data_offset)[..], &u64::to_le_bytes(0x01)[..], // INode Table
            &u64::to_le_bytes(data_offset + 0x100)[..], &u64::to_le_bytes(0x02)[..], // String Table
            &u64::to_le_bytes(data_offset + 0x200)[..], &u64::to_le_bytes(0x01)[..], // History Table
            &u64::to_le_bytes(0x50)[..], &u64::to_le_bytes(meta.len() as u64)[..],
            &meta[..]
        ]
            .into_iter()
            .flatten()
            .cloned()
            .collect()
        );
        
        let mut raw_header = vec![0u8; 0x50];
        header.read_exact(&mut raw_header)?;
        
        Ok(Database {
            raw_header: raw_header.to_vec(),
            backing: Rc::new(RefCell::new(header)),
            inode_table_range: (data_offset, 1),
            string_table_range: (data_offset + 0x100, 2),
            history_table_range: (data_offset + 0x200, 1),
            inode_table: vec![("/".to_string(), PageDescriptor {
                name: "/".to_string(),
                access_control_list: vec![crate::Access::ReadWriteExecute("*".to_string())],
                modified: SystemTime::now(),
                created: SystemTime::now(),
                inodes: vec![]
            })]
                .into_iter()
                .collect(),
            // Upon serialisation, the missing strings will be inserted into the string table, but for completeness' sake, include them here.
            string_table: RefCell::new(vec!["/".to_string(), "*".to_string()]),
           meta: metadata
        })
    }
    
    fn get_strtab_index(&self, str: &String) -> Result<u64> {
        let mut cell = self.string_table.try_borrow_mut()
            .map_err(Error::other)?;
        
        Ok(match cell
            .iter()
            .position(|i| i == str)
            .map(|i| i as u64) {
            Some(index) => index,
            None => {
                cell.push(str.clone());
                cell.len() as u64 - 1
            }
        })
    }
    
    fn parse_string_table(mut backing: RefMut<Buffer>, start: u64, len: u64) -> Result<Vec<String>> {
        let mut buf = Cursor::new(vec![0u8; 512]);
        let mut strtab = vec![];
        backing.seek(std::io::SeekFrom::Start(start))?;
        
        // TODO: If an EOF is reached while attempting to fill the buffer, despite the potential validity of the descriptors, we will receive an error. 
        while strtab.len() < len as usize {
            let buffer = {
                let mut buffer = Vec::new();
                buffer.extend(buf.get_ref());
                buffer.reserve(512);
                buffer[buf.get_ref().len()..].fill(0x00);
                backing.read_exact(&mut buffer[buf.get_ref().len()..])?;
                buffer
            };
            
            let strlen = u16::from_le_bytes(buffer[0..2].try_into().map_err(Error::other)?);
            
            let total_space = 2 + strlen as usize;
            
            strtab.push(String::from_utf8(buffer[2..total_space].to_owned()).map_err(Error::other)?);
            
            buf.seek(SeekFrom::Start(0))?;
            buf.write_all(&buffer[total_space..])?;
        }
        
        Ok(strtab)
    }
        
    pub(crate) fn get_string_table(&mut self) -> Result<Vec<String>> {
        Self::parse_string_table(self.backing.try_borrow_mut()
            .map_err(Error::other)?, self.string_table_range.0, self.string_table_range.1)
    }
    
    pub fn create_page<'a, Path: ToString>(&'a mut self, path: Path) -> Result<crate::Page<Buffer, Box<dyn FnMut(u64) -> Result<Vec<(u64, u64)>> + 'a>>> {
        let path = path.to_string();
        
        if self.inode_table.contains_key(&path) {
            return Err(Error::new(std::io::ErrorKind::AlreadyExists, path));
        }
        
        let data = self.allocate_chunks(0x10)?;
    
        Ok(crate::Page {
            db: DBAgent {
                buffer: Rc::clone(&self.backing),
                allocate: Box::new(|size| self.allocate_chunks(size))
            },
            page_descriptor: crate::PageDescriptor {
                name: path,
                access_control_list: [crate::Access::ReadWriteExecute("*".to_owned())].to_vec(),
                modified: SystemTime::now(),
                created: SystemTime::now(),
                inodes: data
            },
            index: 0,
            history: &[] // TODO: Autofill with created event
        })
    }
    
    pub fn open_page<'a, Path: ToString>(&'a mut self, path: Path) -> Result<crate::Page<Buffer, Box<dyn FnMut(u64) -> Result<Vec<(u64, u64)>> + 'a>>> {
        let path = path.to_string();
        
        let page = self.inode_table.get(&path)
            .ok_or(Error::new(std::io::ErrorKind::NotFound, path))?
            .clone();
            
        Ok(crate::Page {
            db: DBAgent {
                buffer: Rc::clone(&self.backing),
                allocate: Box::new(|size| self.allocate_chunks(size))
            },
            page_descriptor: page,
            index: 0,
            history: &[] // TODO: fetch history
        })
    }
    
    fn parse_inode_table(mut backing: RefMut<Buffer>, start: u64, len: u64) -> Result<HashMap<String, PageDescriptor>> {
        let mut page_table = HashMap::new();
        backing.seek(std::io::SeekFrom::Start(start))?;
        let mut buf = Cursor::new(Vec::new());
        
        // TODO: If an EOF is reached while attempting to fill the buffer, despite the potential validity of the descriptors, we will receive an error. 
        while page_table.len() < len as usize {            
            // Join as an optimisation to reduce the number of reads we have to do. 
            // Since reading in 512 byte increments allows us to have a more predictable read call amount, 
            // we have to account for the fact that each read may yield anywhere between partial and multiple page descriptors.
            // As such we're creating a buffer which is persisted between calls and used in the next call. 
            // This potentially enlarged buffer is parsed-to-end, and any bytes which extend beyond the expected end of the inode table are disregarded, as we can always re-read them. 
            let mut buffer = {
                let mut buffer = Vec::new();
                buffer.extend(buf.get_ref());
                buffer.reserve(512);
                buffer.extend_from_slice(&vec![0x00; buffer.capacity() - buf.get_ref().len()]);
                backing.read_exact(&mut buffer[buf.get_ref().len()..])?;
                buffer
            };
            
            // At this point the buffer will be at least 512 bytes long
            let page_name = u64::from_le_bytes(buffer[0..8].try_into().map_err(Error::other)?);
            let acl_len = u16::from_le_bytes(buffer[8..10].try_into().map_err(Error::other)?) as u64;
            
            let chunks_offset = 10 + (1 + 8) * acl_len;
            
            // since we now know at least how much more space we need, we'll increase the buffer to that amount, and fill it
            buffer.reserve(chunks_offset as usize + 8 - buffer.len());
            // since the buffer already contains 512 bytes, the extra capacity from before will be unreserved, hence, by subtracting the difference between those, we know the remaining volume to fill
            let range = buffer.len()..buffer.capacity();
            backing.read_exact(&mut buffer[range])?;
            
            // ACLs are (u8, u64) pairs which act as hints to the database to regulate access control.
            // The values represent a customisable permission system, allowing specifying up to 8 unrelated permissions, 
            // as well as an index into the string table, which can be used for any purpose such as an email address. 
            let acl: Vec<crate::Access> = buffer[10..chunks_offset as usize]
                .chunks(16)
                .map(|i| match i[0] {
                    0b000 => crate::Access::None(format!("")), // TODO: Replace with strtab lookup
                    0b001 => crate::Access::Read(format!("")), // TODO: Replace with strtab lookup
                    0b011 => crate::Access::ReadWrite(format!("")), // TODO: Replace with strtab lookup
                    0b111 => crate::Access::ReadWriteExecute(format!("")), // TODO: Replace with strtab lookup
                    0b101 => crate::Access::ReadExecute(format!("")), // TODO: Replace with strtab lookup
                    perm => crate::Access::Custom(format!(""), perm), // TODO: Replace with strtab lookup
                })
                .collect();
                
            // realign cursor to nearest 16th byte
            let chunks_offset = chunks_offset + (0x10 - ((1 + 8) * acl_len % 0x10));
            
            let chunk_len = u64::from_le_bytes(buffer[chunks_offset as usize..chunks_offset as usize + 8].try_into().map_err(Error::other)?);
            let total_space = chunks_offset + 8 + (2 * 8 * chunk_len);
            
            buffer.reserve(total_space as usize - buffer.len());
            let range = buffer.len()..buffer.capacity();
            backing.read_exact(&mut buffer[range])?;
            
            // This table contains an array of (offset,length) pairs, each of the u64 type. The bytes expressed by these ranges, concatenated together form the page content. 
            let chunks: Vec<(u64, u64)> = buffer[chunks_offset as usize + 8..total_space as usize]
                .chunks(16)
                .map(|i| Ok((
                    u64::from_le_bytes(i[0..8].try_into().map_err(Error::other)?),
                    u64::from_le_bytes(i[8..16].try_into().map_err(Error::other)?)
                )))
                .collect::<Result<Vec<(u64, u64)>>>()?;
            
            let page_name = format!("Page: {}", page_name); // TODO replace with strtab lookup
            if let Some(page) = page_table.insert(page_name.to_owned(), PageDescriptor {
                name: page_name,
                access_control_list: acl,
                inodes: chunks,
                modified: SystemTime::now(), // TODO: Replace with history table lookup
                created: SystemTime::now() // TODO: Replace with history table lookup
            }) {
                return Err(Error::other(format!("Duplicate page name found: {}", page.name)));
            }
            
            buf.seek(SeekFrom::Start(0))?;
            buf.write_all(&buffer[total_space as usize..])?;
        }
        
        Ok(page_table)
    }
    
    pub fn write_header(&mut self) -> Result<()> {
        let offset = {
            let mut backing = self.backing
                .try_borrow_mut()
                .map_err(Error::other)?;
                
            backing.seek(SeekFrom::Start(0))?;
            backing.write_all(&self.raw_header)?;
            
            backing.seek(SeekFrom::Start(0x50))?;
            let metadata = ron::ser::to_writer(backing.deref_mut(), &self.meta)
                .map_err(Error::other)?;
            
            let offset = backing.seek(SeekFrom::Current(0))?;
            
            offset + (0x10 - offset % 0x10) // Align to next 0x10th byte
        };
        
        // Write INode Table before writing offsets as it may alter the string table
        
        let backing = Rc::clone(&self.backing);
        let mut backing = backing.try_borrow_mut().map_err(Error::other)?;
        
        // Write Header
        backing.seek(SeekFrom::Start(0x10))?;
        // ranges:
        let inode_offset = offset;
        let inode_length = self.inode_table.len() as u64;
        
        backing.seek(SeekFrom::Start(inode_offset))?;
        let data = self.serialise_inode_table()?;
        backing.write_all(&data)?;
        
        let string_offset = (inode_offset + data.len() as u64) + 0x100 & !0x100; // Align to next 0x100th byte
        let string_length = self.string_table.borrow().len() as u64;
        
        backing.seek(SeekFrom::Start(string_offset));
        let data = self.serialise_string_table()?;
        backing.write_all(&data)?;
        
        let history_offset = string_offset + string_length + 0x100 - (string_offset + string_length) % 0x100; // Align to next 0x100th byte
        let history_length = 0;
        
        backing.write_all(&vec![inode_offset, inode_length, string_offset, string_length, history_offset, history_length]
            .into_iter()
            .map(|i| i.to_le_bytes())
            .flatten()
            .collect::<Vec<_>>())?;
        
        Ok(())
    }
    
    fn serialise_inode_table(&self) -> Result<Vec<u8>> {
        let mut vec = vec![];
        
        for (name, page) in self.inode_table.iter().map(|i| (i.0.clone(), i.1.clone())) {
            let strtab_index = self.get_strtab_index(&name)?;
                
            let acl_len = page.access_control_list.len() as u64;
            let acls: Vec<_> = page.access_control_list
                .iter()
                .map(|i| Ok(match i {
                    crate::Access::None(entity) => (0b000u8, self.get_strtab_index(&entity)?),
                    crate::Access::Read(entity) => (0b001u8, self.get_strtab_index(&entity)?),
                    crate::Access::ReadWrite(entity) => (0b011u8, self.get_strtab_index(&entity)?),
                    crate::Access::ReadWriteExecute(entity) => (0b111u8, self.get_strtab_index(&entity)?),
                    crate::Access::ReadExecute(entity) => (0b101u8, self.get_strtab_index(&entity)?),
                    crate::Access::Custom(entity, perm) => (*perm, self.get_strtab_index(&entity)?)
                }))
                .collect::<Result<Vec<(u8, u64)>>>()?
                .into_iter()
                .map(|i| {
                    let mut arr = [0u8; 1 + 8];
                    arr[0] = i.0;
                    
                    i.1
                        .to_le_bytes()
                        .into_iter()
                        .enumerate()
                        .for_each(|(a, i)| arr[a + 1] = i);
                        
                    arr
                })
                .flatten()
                .collect();
                
            vec.extend((&[
                &u64::to_le_bytes(self.get_strtab_index(&page.name)?)[..],
                &u64::to_le_bytes(page.access_control_list.len() as u64)[..],
                &acls[..],
                &vec![0x00; 0x10 - (acls.len() * (1 + 8) % 0x10)][..],
                &u64::to_le_bytes(page.inodes.len() as u64)[..],
            ][..])
                .iter()
                .cloned()
                .flatten());
            
            for i in page.inodes.iter().cloned() {
                vec.extend_from_slice(&i.0.to_le_bytes()[..]);
                vec.extend_from_slice(&i.1.to_le_bytes()[..]);
            }
        }
        
        Ok(vec)
    }
    
    fn serialise_string_table(&self) -> Result<Vec<u8>> {
        let mut vec = vec![];
        
        for i in self.string_table.try_borrow().map_err(Error::other)?.iter() {
            vec.extend_from_slice(&[
                &(i.len() as u64).to_le_bytes()[..],
                i.as_bytes()
            ]
                .into_iter()
                .flatten()
                .cloned()
                .collect::<Vec<_>>());
        }
        
        Ok(vec)
    }
    
    // TODO: Refactor to make returning multiple chunks which add up to `min_space`
    pub(crate) fn allocate_chunks(&mut self, min_space: u64) -> Result<Vec<(u64, u64)>> {        
        // TODO: Determine clean way determine space before first inode
        
        let mut inodes = self.inode_table.values()
            .map(|i| i.inodes.iter())
            .flatten()
            .cloned()
            .scan((0u64, 0u64), |a, i| {
                // The gap is the the start of the current + length => the start of the next
                let out = Some((a.0 + a.1, i.0 - (a.0 + a.1)));
                *a = i;
                return out;
            })
            .collect::<Vec<_>>();
        inodes.sort_unstable_by(|i, j| Ord::cmp(&i.1, &j.1));
        
        if let Some(inode) = inodes.iter()
            .find(|i| i.1 >= min_space) {
            Ok(vec![(inode.0, min_space)])
        } else {
            // todo!("Expand file to make room for new chunk")
            let mut backing = self.backing.try_borrow_mut()
                .map_err(Error::other)?;
                
            let position = backing.seek(SeekFrom::End(0))?;
            backing.write_all(&vec![0u8; (min_space + (0x1000 - min_space % 0x1000)) as usize])?;
            
            Ok(vec![(position, min_space)])
        }
    }
    
    pub fn change_buffer<NewBuffer>(self, buffer: NewBuffer) -> Result<Database<NewBuffer, Metadata>> where NewBuffer: Read + Write + Seek {
        let mut db = Database {
            backing: Rc::new(RefCell::new(buffer)),
            inode_table_range: self.inode_table_range,
            string_table_range: self.string_table_range,
            history_table_range: self.history_table_range,
            inode_table: self.inode_table,
            string_table: self.string_table,
            raw_header: self.raw_header,
            meta: self.meta,
        };
        
        db.write_header()?;
        
        Ok(db)
    }
    
    pub(crate) fn leak_string_table(&self) -> Ref<Vec<String>> {
        self.string_table.borrow()
    }
    
    pub(crate) fn leak_inode_table(&self) -> HashMap<String, PageDescriptor> {
        self.inode_table.clone()
    }
}
