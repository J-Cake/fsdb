use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io::Read;
use std::io::Seek;
use std::io::Write;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::Weak;

use crate::Error;
use crate::Mediator;
use crate::PageDescriptor;

pub struct Database {
    backing: Mutex<Mediator>,

    inode_table: HashMap<String, PageDescriptor>,
    string_table: Vec<String>,
    // TODO: Implement journal
    command_receiver: Receiver<crate::PageRequest>,
}

impl Database {
    pub fn change_backing<NewBacking>(self, backing: NewBacking) -> Database<NewBacking>
    where
        NewBacking: Read + Write + Seek,
    {
        todo!()
    }

    pub fn create_page<Str: AsRef<str>>(&mut self, page: Str) -> Result<crate::Page, Error> {
        todo!()
    }
}
