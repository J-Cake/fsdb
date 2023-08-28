use std::io::Error;
use std::io::SeekFrom;
use std::io::Read;
use std::io::Write;
use std::io::Seek;
use std::io::Result;
use std::time::SystemTime;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Metadata about the page it describes.
#[derive(Debug, Clone)]
pub(crate) struct PageDescriptor {
    /// The name of the page (typically a path)
    pub(crate) name: String,
    /// A list of generically-defined access lists. It is up to the caller to interpret these.
    pub(crate) access_control_list: Vec<crate::Access>,
    /// When the page was last modified - determined by querying the journal
    pub(crate) modified: SystemTime,
    /// When the page was created - determined by querying the journal
    pub(crate) created: SystemTime,
    /// A list of chunks ((start, length)) in order 
    pub(crate) inodes: Vec<crate::Array>
}

/// Information about what happened, when
pub enum HistoryEntry<'db> {
    /// The page was created
    Created,
    /// The following range of data was modified 
    Modified {
        start: u64,
        len: u64,
        /// The previous content contained between start and start + len
        content: Option<&'db [u8]>,
        /// The hash of the previous content
        hash: Option<&'db [u8]>
    },
    /// The access list of the page was altered
    AccessModified {
        /// The previous access list
        prev_acl: Vec<crate::Access>
    },
    /// The page's inode table was modified
    INodeListModified {
        /// The previous inode list
        prev_inodes: Vec<crate::Array>
    },
    /// The page was deleted
    Deleted
}

/// A Read/Write handle to the page's underlying data. Analogous to a File
pub struct Page<'db, Buffer>
where 
    Buffer: Read + Write + Seek,
{
    pub(crate) db: crate::DBAgent<Buffer>,
    pub(crate) history: &'db [(SystemTime, HistoryEntry<'db>)],
    pub(crate) page_descriptor: PageDescriptor,
    
    pub(crate) index: u64
}

impl<'db, Buffer> Seek for Page<'db, Buffer> 
where 
    Buffer: Read + Write + Seek,
{
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match pos {
            SeekFrom::Start(start) => self.index = start,
            SeekFrom::Current(offset) => if -offset > self.index as i64 {
                return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Seek beyond beginning"));
            } else {
                self.index = (self.index as i64 + offset) as u64;
            },
            SeekFrom::End(offset) => {
                let total = self.page_descriptor.inodes.iter().map(|i| i.length).sum::<u64>() as i64;
                if -offset > total as i64 {
                    return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Seek beyond end"));
                } else {
                    self.index = (total + offset) as u64;
                }                    
            }
        };
        
        Ok(self.index)
    }
}

impl<'db, Buffer> Read for Page<'db, Buffer> 
where 
    Buffer: Read + Write + Seek,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut running_size: u64 = 0;
        let mut backing = self.db.try_borrow_mut()?;
            
        for crate::Array { length: len, offset: start } in self.page_descriptor.inodes
            .iter() {
                
            // Read at most one chunk
            if running_size + len > self.index {
                backing.seek(SeekFrom::Start(start + (self.index - running_size)))?;
                let len = (*len as usize).min(buf.len());
                backing.read_exact(&mut buf[0..len])?;
                self.index += len as u64;
                return Ok(len);
            }
            
            running_size += len;
        }
        
        Ok(0)
    }
}

impl<'db, Buffer> Write for Page<'db, Buffer> 
where 
    Buffer: Read + Write + Seek,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        {
            let mut running_size: u64 = 0;
            let mut backing = self.db.try_transparent_borrow_mut()?;
                
            for crate::Array { length: len, offset: start } in self.page_descriptor.inodes
                .iter() {
                    
                // Read at most one chunk
                if running_size + len > self.index {
                    backing.seek(SeekFrom::Start(start + (self.index - running_size)))?;
                    let len = (*len as usize).min(buf.len());
                    backing.write_all(&buf[0..len])?;
                    self.index += len as u64;
                    return Ok(len);
                }
                
                running_size += len;
            }
        }
        
        let chunks = self.db.allocate_chunks(buf.len() as u64)?;
        let len = {
            self.page_descriptor.inodes.extend(chunks.iter());
            let mut backing = self.db.try_transparent_borrow_mut()?;
            let first = chunks.first().ok_or(Error::new(std::io::ErrorKind::StorageFull, format!("Required {:?}B more", buf.len())))?;
            
            backing.seek(SeekFrom::Start(first.offset))?;
            
            backing.write_all(&buf[0..first.length as usize])?;
            first.length as usize
        };
        
        self.flush()?;
        
        Ok(len as usize)
    }
    
    fn flush(&mut self) -> std::io::Result<()> {
        todo!();
    }
}
