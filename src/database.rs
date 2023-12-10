use std::collections::HashMap;
use std::io::Read;
use std::io::Seek;
use std::io::Write;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use crate::error::Error;

use crate::mediator::Mediator;
use crate::page::Page;
use crate::page::PageDescriptor;
use crate::page::PageRequest;

pub struct Database<Backing> where Backing: Read + Write + Seek + 'static  {
    backing: Mutex<Mediator<Backing>>,

    inode_table: HashMap<String, Arc<RwLock<PageDescriptor>>>,
    string_table: Vec<String>,
    // TODO: Implement journal
    command_receiver: Receiver<PageRequest>,
}

impl<Backing> Database<Backing> where Backing: Read + Write + Seek + 'static  {
    pub fn change_backing<NewBacking>(self, backing: NewBacking) -> Database<NewBacking>
    where NewBacking: Read + Write + Seek + 'static {
        todo!()
    }

    pub fn create_page<Str: AsRef<str>>(&mut self, page: Str) -> Result<Page<Backing>, Error> {
        todo!()
    }
}