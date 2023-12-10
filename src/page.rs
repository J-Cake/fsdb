use std::cell::Cell;
use std::io::Read;
use std::io::Seek;
use std::io::Write;
use std::marker::PhantomData;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;

use crate::access::Access;
use crate::error::Error;
use crate::format::Array;
use crate::mediator::Mediator;

/// Metadata about the page it describes.
#[derive(Debug, Clone)]
pub(crate) struct PageDescriptor {
    /// The name of the page (typically a path)
    pub(crate) name: String,
    /// A list of generically-defined access lists. It is up to the caller to interpret these.
    pub(crate) access_control_list: Vec<Access>,
    /// When the page was last modified - determined by querying the journal
    pub(crate) modified: SystemTime,
    /// When the page was created - determined by querying the journal
    pub(crate) created: SystemTime,
    /// A list of chunks ((start, length)) in order
    pub(crate) inodes: Vec<Array>,
}

pub enum SpaceRequirements {
    GrowBy(u64),
    SetLen(u64),
}

pub enum ACLOperation {
    Add(Access),
    Remove(Access),
    Alter(Access),
}

pub enum PageRequest {
    RefreshChunks,
    AllocateSpace(SpaceRequirements),
    ChangeACL(ACLOperation),
    Close,
}

enum Response {
    Ok,
    Busy,
    NotPermitted,
}

pub struct PageResponse {
    request: PageRequest,
    response: Response,
}

pub struct ReadStream<Data: AsRef<[u8]>> {
    chunk_size: usize,
    buffer: Vec<u8>,
    inodes: Vec<Array>,
    data: PhantomData<Data>
}

impl<Data: AsRef<[u8]>> Iterator for ReadStream<Data> {
    type Item = Data;
    
    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

/// Pages represent logical units of data which can be opened, read and written to within the database. 
/// They contain various metadata, as well as a list of chunks whose concatenation forms the page's contents.
pub struct Page<Backing> where Backing: Read + Write + Seek + 'static {
    /// The page descriptor is a struct which contains all the information associated with a page. 
    /// It includes information about the page's access permissions, it's journal as well as the list of chunks the page is to consume.
    descriptor: PageDescriptor,
    
    /// Pages are expected to buffer their content for faster read/write. 
    /// The buffer may be size-constrained by the database's configuration object (metadata), or contain the entire page
    large_buffer: Mutex<Cell<Vec<u8>>>,

    /// The structure which regulates and manages read/write access to various chunks of the backing object.
    /// It uses atomic primitives internally to ensure synchronous locking, and can therefore be passed around immutably.
    mediator: Arc<Mediator<Backing>>
}

impl<Backing> Page<Backing> where Backing: Read + Write + Seek + 'static {
    pub fn len(&self) -> usize {
        self.descriptor
            .inodes
            .iter()
            .map(|i| i.length)
            .sum::<u64>() as usize
    }
    
    pub fn read_all(&self) -> Result<(), Error> {
        let chunks = &self.descriptor
            .inodes;
        
        Ok(())
    }
    
    pub fn read_stream<Data: AsRef<[u8]>>(&self) -> Result<ReadStream<Data>, Error> {
        todo!()
    }
    
    pub fn write_stream<Iter: Iterator<Item=Source>, Source: AsRef<[u8]>>(&mut self, content: Iter) -> Result<(), Error> {
        todo!()
    }
    
    pub fn flush(&mut self) -> Result<(), Error> {
        todo!()
    }
    
    pub fn close(&mut self) {
        todo!()
    }
}

impl<Backing> Drop for Page<Backing> where Backing: Read + Write + Seek + 'static  {
    fn drop(&mut self) {
        self.close();
    }
}

impl<Backing> AsRef<[u8]> for Page<Backing> where Backing: Read + Write + Seek + 'static  {
    fn as_ref(&self) -> &[u8] {
        // self.large_buffer.borrow()
        todo!()
    }
}

impl<Backing> AsMut<[u8]> for Page<Backing> where Backing: Read + Write + Seek + 'static  {
    fn as_mut(&mut self) -> &mut [u8] {
        // self.large_buffer.borrow_mut()
        todo!()
    }
}

#[cfg(feature = "rwpage")]
impl<Backing> Read for Page<Backing> where Backing: Read + Write + Seek + 'static  {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        todo!()
    }
}

#[cfg(feature = "rwpage")]
impl<Backing> Write for Page<Backing> where Backing: Read + Write + Seek + 'static  {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        todo!()
    }

    fn flush(&mut self) -> std::io::Result<()> {
        todo!()
    }
}

#[cfg(feature = "rwpage")]
impl<Backing> Seek for Page<Backing> where Backing: Read + Write + Seek + 'static  {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        todo!()
    }
}
