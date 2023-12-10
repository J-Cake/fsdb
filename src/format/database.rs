use std::collections::HashMap;
use std::io::BufReader;
use std::io::Cursor;
use std::io::Error;
use std::io::Result;
use std::time::SystemTime;
use std::cmp::Ordering;
use std::iter;
use std::rc::Rc;
use std::cell::{RefCell, RefMut, Ref};
use std::io::{Read, Write, Seek, SeekFrom};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::access::Access;
use crate::format::array::{Array, round};
use crate::page::PageDescriptor;

#[macro_export]
macro_rules! get_str {
    ($strtab:expr, $n:expr) => ($strtab.get($n as usize).ok_or(Error::new(std::io::ErrorKind::NotFound, format!("No string found for index {}", $n))));
}

/// Contains information about the database, providing a clean interface to accessing it.
/// This object represents the on-disk parseable format which can be transformed into a live Database object for consumption.
pub struct Database<Buffer, Metadata> where Buffer: Read + Write + Seek, Metadata: Serialize + DeserializeOwned + Clone {
    /// The underlying data source. As long as it supports Read, Write and Seek operations, this can be anything.
    pub(crate) backing: Rc<RefCell<Buffer>>,
    /// Number of elements in inode table + Offset
    pub(crate) inode_table_range: Array,
    /// Number of elements in string table + Offset
    pub(crate) string_table_range: Array,
    /// Number of elements in history table + Offset
    pub(crate) history_table_range: Array,
    /// Number of elements in history table + Offset
    pub(crate) metadata_range: Array,
    
    inode_table: HashMap<String, PageDescriptor>,
    string_table: RefCell<Vec<String>>,
    
    inode_table_size: u64,
    string_table_size: u64,
    history_table_size: u64,
    
    borrowed_slices: Arc<Mutex<Vec<Array>>>,
    
    raw_header: Vec<u8>,
    pub meta: Metadata
}

impl<Backing, Metadata> Database<Backing, Metadata> where Backing: Read + Write + Seek, Metadata: Serialize + DeserializeOwned + Clone {
    /// Parse the backing buffer into a Database object.
    /// ```rust
    /// struct Metadata {
    ///     pub friendly_name: String,
    ///     pub max_chunk_size: u64,
    ///     pub chunk_alignment: u64,
    ///     pub max_page_size: u64,
    ///     pub page_alignment: u64,
    /// }
    ///
    /// use datastore_provider::format::database::Database;
    /// let file = std::fs::OpenOptions::new()
    ///     .read(true)
    ///     .write(true)
    ///     .open("./test-file.db")?;
    ///
    /// Database::<std::fs::File, Metadata>::open(file)?;
    /// ```
    /// > **Note**: The `Metadata` structure is completely arbitrary, and the database does not interpret nor otherwise use its values in any way.
    ///     It's designed to act as a preferences map for use by consumers or hooks of the database.
    pub fn open(mut backing: Backing) -> Result<Self> {
        let mut reader = BufReader::new(&mut backing);
        reader.seek(std::io::SeekFrom::Start(0))?;

        let mut buf = vec![0u8; 4 + 4 + 4 + 4 + (4 * (2 * 8))];
        reader.read_exact(&mut buf)?;
        if &buf[0..4] != b"FSDB" { return Err(Error::other("Invalid Magic Number")); }
        if buf[4..8] != [0x01, 0, 0, 0] { return Err(Error::other("Unrecognised version")); }

        let inode_table_range = Array {
            length: u64::from_le_bytes(buf[16..24]
                .try_into()
                .map_err(Error::other)?),
            offset: u64::from_le_bytes(buf[24..32]
                .try_into()
                .map_err(Error::other)?)
        };

        let string_table_range = Array {
            length: u64::from_le_bytes(buf[32..40]
                .try_into()
                .map_err(Error::other)?),
            offset: u64::from_le_bytes(buf[40..48]
                .try_into()
                .map_err(Error::other)?)
        };

        let history_table_range = Array {
            length: u64::from_le_bytes(buf[48..56]
                .try_into()
                .map_err(Error::other)?),
            offset: u64::from_le_bytes(buf[56..64]
                .try_into()
                .map_err(Error::other)?)
        };

        let metadata_range = Array {
            length: u64::from_le_bytes(buf[64..72]
                    .try_into()
                    .map_err(Error::other)?),
            offset: u64::from_le_bytes(buf[72..80]
                    .try_into()
                    .map_err(Error::other)?)
        };

        let backing = Rc::new(RefCell::new(backing));

        let strtab = Self::parse_string_table(Rc::clone(&backing)
            .try_borrow_mut()
            .map_err(Error::other)?, string_table_range)?;
        let string_table_size = strtab.len() as u64;
        let strtab = RefCell::new(strtab);

        let inodetab = Self::parse_inode_table(Rc::clone(&backing)
            .try_borrow_mut()
            .map_err(Error::other)?, strtab.borrow(), inode_table_range)?;

        let x = Ok(Self {
            inode_table_size: inodetab.len() as u64,
            string_table_size,
            history_table_size: 0,

            inode_table: inodetab,
            string_table: strtab,

            inode_table_range,
            string_table_range,
            history_table_range,
            metadata_range,

            borrowed_slices: Arc::new(Mutex::new(vec![])),

            raw_header: buf.clone(),
            meta: {
                let mut s = vec![0u8; metadata_range.length as usize];
                let mut backing: RefMut<Backing> = backing
                    .try_borrow_mut()
                    .map_err(Error::other)?;

                backing.seek(SeekFrom::Start(metadata_range.offset))?;
                backing.read_exact(&mut s)?;

                ron::de::from_bytes::<Metadata>(&s)
                    .map_err(Error::other)?
                    .clone()
            },

            backing: Rc::clone(&backing),
        });

        return x;
    }

    /// Compute the offset of the allowable data region.
    fn data_offset(&self) -> u64 {
        (self.inode_table_range.offset + self.inode_table_size)
            .max(self.string_table_range.offset + self.string_table_size)
            // .max(self.history_table_range.offset + self.history_table_size) // TODO: Include once History Table becomes relevant
            .max(self.metadata_range.offset + self.metadata_range.length)
    }

    /// Fetch a string in the string table
    /// Strings are referenced by their index into the table, and can be easily fetched using the `str!` macro:
    /// ```rust
    /// fn get_string_by_index(index: u64, strtab: std::cell::Ref<Vec<String>>) -> Option<String> {
    ///     let str = datastore_provider::get_str!(strtab, index).ok()?.clone();
    ///     Some(str)
    /// }
    /// ```
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

    /// Read the contents of the string table into a vector
    fn parse_string_table(mut backing: RefMut<Backing>, arr: Array) -> Result<Vec<String>> {
        let mut buf = Cursor::new(vec![0u8; 512]);
        let mut strtab = vec![];
        let offset = backing.seek(std::io::SeekFrom::Start(arr.offset))?;

        // TODO: If an EOF is reached while attempting to fill the buffer, despite the potential validity of the descriptors, we will receive an error.
        while strtab.len() < arr.length as usize {
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

    /// Parse the string table.
    pub(crate) fn get_string_table(&mut self) -> Result<Vec<String>> {
        Self::parse_string_table(self.backing.try_borrow_mut()
            .map_err(Error::other)?, self.string_table_range)
    }

    /// Parse the inode table
    fn parse_inode_table(mut backing: RefMut<Backing>, strtab: Ref<Vec<String>>, arr: Array) -> Result<HashMap<String, PageDescriptor>> {
        let mut buf = BufReader::new(backing.deref_mut());
        let mut map = HashMap::new();

        let strtab = strtab.deref();

        let offset = buf.seek(SeekFrom::Start(arr.offset))?;

        while (map.len() as u64) < arr.length {
            // Read the necessary information first.

            // u64 + u16
            let mut page_header = [0u8; 8 + 2];
            buf.read_exact(&mut page_header)?;

            let page_name = u64::from_le_bytes(page_header[0..8].try_into().map_err(Error::other)?);
            let acl_len = u16::from_le_bytes(page_header[8..10].try_into().map_err(Error::other)?) as u64;

            // (u8 + u64) * acl_len + %0x10
            let mut acl = vec![0u8; round((1 + 8) * acl_len as u64, 0x10) as usize - 2];
            buf.read_exact(&mut acl)?;

            // u64
            let mut chunk_len = [0u8; 8];
            buf.read_exact(&mut chunk_len)?;

            let chunk_len = u64::from_le_bytes(chunk_len);

            // (u64 + u64) * chunk_len
            let mut chunk_ranges = vec![0u8; 2 * 8 * chunk_len as usize];
            buf.read_exact(&mut chunk_ranges)?;

            let name: &String = get_str!(strtab, page_name)?;

            map.insert(
                name.clone(),
                PageDescriptor {
                    name: name.clone(),
                    access_control_list: acl[0..(1 + 8) * acl_len as usize]
                        .chunks(1 + 8) // u8 + u64
                        .map(|i| Ok(match i[0] {
                            0b000 => Access::None(get_str!(strtab, i[1])?.clone()),
                            0b001 => Access::Read(get_str!(strtab, i[1])?.clone()),
                            0b011 => Access::ReadWrite(get_str!(strtab, i[1])?.clone()),
                            0b111 => Access::ReadWriteExecute(get_str!(strtab, i[1])?.clone()),
                            0b101 => Access::ReadExecute(get_str!(strtab, i[1])?.clone()),
                            perm => Access::Custom(get_str!(strtab, i[1])?.clone(), perm),
                        }))
                        .collect::<Result<Vec<Access>>>()?,
                    inodes: chunk_ranges
                        .chunks(8 + 8) // u64 + u64
                        .map(|i| Ok(Array {
                            length: u64::from_le_bytes(i[0..8].try_into().map_err(Error::other)?),
                            offset: u64::from_le_bytes(i[8..16].try_into().map_err(Error::other)?)
                        }))
                        .collect::<Result<Vec<Array>>>()?,
                    modified: SystemTime::now(),
                    created: SystemTime::now(),
                }
            );
        }

        Ok(map)
    }

    /// Serialise the header into the defined format and write it to the backing buffer.
    /// Open pages will automatically synchronise their changes with the header and usually don't need manual flushing.
    /// This method is mainly used internally, but can be additionally invoked for extra clarity or assurance.
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

        backing.seek(SeekFrom::Start(string_offset))?;
        let data = self.serialise_string_table()?;
        backing.write_all(&data)?;

        let history_offset = string_offset + string_length + 0x100 - (string_offset + string_length) % 0x100; // Align to next 0x100th byte
        let history_length = 0;

        backing.write_all(&vec![inode_length, inode_offset, string_length, string_offset, history_length, history_offset]
            .into_iter()
            .map(|i| i.to_le_bytes())
            .flatten()
            .collect::<Vec<_>>())?;

        Ok(())
    }

    /// Generate a byte buffer of the inode table
    fn serialise_inode_table(&mut self) -> Result<Vec<u8>> {
        let mut vec = vec![];

        for (name, page) in self.inode_table.iter().map(|i| (i.0.clone(), i.1.clone())) {
            let strtab_index = self.get_strtab_index(&name)?;

            let acl_len = page.access_control_list.len() as u64;
            let acls: Vec<_> = page.access_control_list
                .iter()
                .map(|i| Ok(match i {
                    Access::None(entity) => (0b000u8, self.get_strtab_index(&entity)?),
                    Access::Read(entity) => (0b001u8, self.get_strtab_index(&entity)?),
                    Access::ReadWrite(entity) => (0b011u8, self.get_strtab_index(&entity)?),
                    Access::ReadWriteExecute(entity) => (0b111u8, self.get_strtab_index(&entity)?),
                    Access::ReadExecute(entity) => (0b101u8, self.get_strtab_index(&entity)?),
                    Access::Custom(entity, perm) => (*perm, self.get_strtab_index(&entity)?)
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
                &u16::to_le_bytes(page.access_control_list.len() as u16)[..],
                &acls[..],
                &vec![0x00; round(2 + (1 + 8) * acls.len() as u64, 0x10) as usize][..],
                &u64::to_le_bytes(page.inodes.len() as u64)[..],
            ][..])
                .iter()
                .cloned()
                .flatten());

            for i in page.inodes.iter().cloned() {
                vec.extend_from_slice(&i.length.to_le_bytes()[..]);
                vec.extend_from_slice(&i.offset.to_le_bytes()[..]);
            }
        }

        self.inode_table_size = vec.len() as u64;
        Ok(vec)
    }

    /// Generate a byte-buffer of the string table.
    fn serialise_string_table(&mut self) -> Result<Vec<u8>> {
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

        self.string_table_size = vec.len() as u64;
        Ok(vec)
    }

    /// Generate a byte-buffer of the history table
    /// **!Not Implemented**
    fn serialise_history_table(&mut self) -> Result<Vec<u8>> {
        self.history_table_size = 0;
        Ok(vec![])
    }

    // TODO: Refactor to make returning multiple chunks which add up to `min_space` possible
    /// Request the backing object grow by `min_space` bytes.
    /// This is used before appending chunks to a page, and ensures that unused chunks are either reused, deleted or reallocated before being assigned to a page.
    fn allocate_chunks(&mut self, min_space: u64) -> Result<Vec<Array>> {
        let total_length: u64 = self.backing.try_borrow_mut()
            .map_err(Error::other)?
            .deref_mut()
            .stream_len()? as u64;

        let mut inodes = self.inode_table.values()
            .map(|i| i.inodes.iter())
            .flatten()
            .cloned()
            .chain(iter::once(Array { length: 0, offset: self.data_offset() }))
            .chain(iter::once(Array { length: 0, offset: total_length }))
            .collect::<Vec<_>>();

        inodes.sort_unstable_by(|i, j| Ord::cmp(&i.offset, &j.offset));

        let mut inodes = inodes
            .into_iter()
            .scan(Array { length: 0u64, offset: self.data_offset() }, |a, i| {
                // The gap is the the start of the current + length => the start of the next
                let out = Some(Array {
                    length: i.offset - (a.offset + a.length),
                    offset: a.offset + a.length
                });
                *a = i;
                return out;
            })
            .collect::<Vec<_>>();
        inodes.sort_unstable_by(|i, j| Ord::cmp(&i.length, &j.length));

        if let Some(inode) = inodes.iter()
            .find(|i| i.length >= min_space) {
            Ok(vec![Array { offset: inode.offset, length: min_space }])
        } else {
            // todo!("Expand file to make room for new chunk")
            let mut backing = self.backing.try_borrow_mut()
                .map_err(Error::other)?;

            let position = backing.seek(SeekFrom::End(0))?;
            backing.write_all(&vec![0u8; (min_space + (0x1000 - min_space % 0x1000)) as usize])?;

            Ok(vec![Array {offset: position, length: min_space }])
        }
    }

    /// Grow a page by at least `min_space` bytes. Usually involves appending a new chunk to the page, but can also cause the final chunk to grow.
    fn grow(&mut self, page: &PageDescriptor, min_space: u64) -> Result<()> {
        if let Some(page) = self.inode_table.get_mut(&page.name) {

        }

        Ok(())
    }

    /// Swap the backing object against any new container. Useful for cloning / duplicating parts or all of the database, or initialising new databases on blank containers.
    /// ```rust
    /// let container = std::fs::OpenOptions::new()
    ///     .read(true)
    ///     .write(true)
    ///     .open("/tmp/db.db")?;
    ///
    ///
    /// use datastore_provider::format::database::Database;#[derive(Default)]
    /// struct Metadata {
    ///     pub friendly_name: String,
    ///     pub max_chunk_size: u64,
    ///     pub chunk_alignment: u64,
    ///     pub max_page_size: u64,
    ///     pub page_alignment: u64,
    /// }
    ///
    /// // initialise a new database with a backing vector (completely in-memory), wrapped in a Cursor for `Seek`ability.
    /// let db: Database<std::io::Cursor<Vec<u8>>, Metadata> = Database::blank(Metadata::default());
    /// let db: Database<std::fs::File, Metadata> = db.change_buffer(container)?;
    /// ```
    pub fn change_buffer<NewBuffer>(self, buffer: NewBuffer) -> Result<Database<NewBuffer, Metadata>> where NewBuffer: Read + Write + Seek {
        let mut db = Database {
            backing: Rc::new(RefCell::new(buffer)),
            inode_table_range: self.inode_table_range,
            string_table_range: self.string_table_range,
            history_table_range: self.history_table_range,
            inode_table_size: self.inode_table_size,
            string_table_size: self.string_table_size,
            history_table_size: self.history_table_size,
            metadata_range: self.metadata_range,
            inode_table: self.inode_table,
            string_table: self.string_table,
            raw_header: self.raw_header,
            meta: self.meta,
            borrowed_slices: Arc::new(Mutex::new(vec![])),
        };

        // flush the header to keep the new backing object in-sync
        db.write_header()?;

        Ok(db)
    }

    /// Gain a sneaky reference to the string table. Useful during parsing or serialisation
    pub(crate) fn leak_string_table(&self) -> Ref<Vec<String>> {
        self.string_table.borrow()
    }

    /// Gain a sneaky reference to the inode table. Useful during parsing or seralisation
    pub(crate) fn leak_inode_table(&self) -> HashMap<String, PageDescriptor> {
        self.inode_table.clone()
    }
}