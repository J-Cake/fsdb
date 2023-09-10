use std::io::Error;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::io::Seek;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::mpsc::Sender;
use std::sync::mpsc::Receiver;
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

pub enum SpaceRequirements {
    GrowBy(u64),
    SetLen(u64)
}

pub enum ACLOperation {
    Add(crate::Access),
    Remove(crate::Access),
    Alter(crate::Access)
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
    response: Response
}

pub struct Page<Backing, Metadata>
where
    Backing: Read + Write + Seek,
    Metadata: Serialize + DeserializeOwned + Clone 
{
    pub(crate) chunks: Arc<Mutex<Vec<crate::MutexChunk<Backing>>>>,
    pub(crate) sender: Sender<PageRequest>,
    pub(crate) receiver: Receiver<PageResponse>,
    pub(crate) page_descriptor: Arc<Mutex<PageDescriptor>>,
    
    offset: u64
}

impl<Backing, Metadata> Page<Backing, Metadata>
where
    Backing: Read + Write + Seek,
    Metadata: Serialize + DeserializeOwned + Clone
{
    
}

impl<Backing, Metadata> Read for Page<Backing, Metadata> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // TODO: Optimise away unnecessary mutex locks
        let chunks = match self.chunks.lock() {
                Ok(chunk) => chunk,
                Err(err) => return Error::new(ErrorKind::ResourceBusy, format!("Page busy"))
            }.iter()
            .map(|i| i.lock().map_err(|| Error::new(ErrorKind::ResourceBusy, format!("Page busy"))))
            .collect::<Result<Vec<crate::Chunk<Backing>>>>()?
            .into_iter()
            .skip_while(|i| i.bounds.offset + i.bounds.length > self.offset);
            
        let mut buf_offset = 0usize;
        while buf_offset < buf.len() {
            if let Some(chunk) = chunks.next() {
                let start: usize = (self.offset - chunk.bounds.offset) as usize;
                let len: usize = chunk.buffer.len().min((self.offset as usize + buf.len()) - start);
                let src = &chunk.buffer[start..start + len];
                
                (&mut buf[buf_offset..src.len()]).copy_from_slice(&src);
                buf_offset += src.len();
                self.offset += src.len();
            } else {
                return Ok(buf_offset)
            }
        }
        
        Ok(buf_offset)
    }
}

// TODO: Write + Seek impls

