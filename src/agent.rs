use std::cell::Ref;
use std::cell::RefMut;
use std::io::Error;
use std::io::Read;
use std::io::Write;
use std::io::Seek;
use std::io::Result;
use std::marker::PhantomData;
use std::rc::Rc;
use std::cell::RefCell;

use crate::Array;
use crate::PageDescriptor;

/// A proxy which provides a reading and writing interface to the database's buffer.
#[derive(Clone)]
pub(crate) struct DBAgent<Buffer> 
where 
    Buffer: Read + Write + Seek,
{
    buffer: Rc<RefCell<Buffer>>,
}


impl<Buffer> DBAgent<Buffer> 
where 
    Buffer: Read + Write + Seek,
{
    pub fn new(buffer: Buffer) -> Self {
        Self { 
            buffer: Rc::new(RefCell::new(buffer))
        }
    }
    
    pub fn from_existing(buffer: Rc<RefCell<Buffer>>) -> Self {
        Self {
            buffer
        }
    }
    
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
    
    pub fn allocate_chunks(&mut self, min_size: u64) -> Result<Vec<Array>> {
        // TODO: Implement typed + returnable message-passing system
        todo!();
    }
}
