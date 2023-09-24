use std::time::SystemTime;
use std::io::Seek;
use std::io::Read;
use std::io::Write;

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

pub struct Page {
    descriptor: PageDescriptor
}

impl Page {
    
}

#[cfg(feature = "rwpage")]
impl Read for Page {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        todo!()
    }
}

#[cfg(feature = "rwpage")]
impl Write for Page {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        todo!()
    }
    
    fn flush(&mut self) -> std::io::Result<()> {
        todo!()
    }
}

#[cfg(feature = "rwpage")]
impl Seek for Page {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        todo!()
    }
}
