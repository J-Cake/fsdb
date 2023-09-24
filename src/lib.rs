#![feature(io_error_other)]
#![feature(io_error_more)]
#![feature(write_all_vectored)]
#![feature(seek_stream_len)]

pub mod database;
pub mod page;
pub mod access;
pub mod agent;
pub mod format;
pub mod error;
pub(crate) mod mediator;

pub use database::*;
pub use access::*;
pub use page::*;
pub use format::*;
pub use error::*;
pub(crate) use mediator::*;

#[cfg(test)]
pub mod test {
    use std::fs::File;
    use std::fs::OpenOptions;
    use std::io::Error;
    use std::io::Result;
    use std::io::Cursor;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;
    use serde::Serialize;
    use serde::Deserialize;
    
    #[derive(Debug, Serialize, Deserialize, Clone)]
    struct Metadata {
        max_page_size: u64,
        max_chunk_size: u64,
        max_journal_size: u64,
        reallocation_volume: usize
    }
    
    impl Default for Metadata {
        fn default() -> Self {
            Self {
                max_page_size: 0x1_000_000_000,
                max_chunk_size: 0x1_000_000,
                max_journal_size: 100,
                reallocation_volume: 0x1000 // 1KiB
            }
        }
    }
    
    #[test]
    pub fn blank() -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open("/tmp/test.db")?;
            
        let mut blank = crate::blank::<Metadata>()?
            .change_backing(file);
            
        Ok(())
    }
    
    #[test]
    pub fn create_page() -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open("/tmp/test.db")?;
        
        let mut blank = crate::blank::<Metadata>()?
            .change_backing(file);
            
        let page = blank.create_page("test")?;
        
        Ok(())
    }
    
    #[test]
    pub fn read_write() -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open("/tmp/test.db")?;
        
        let mut blank = crate::Database::<Cursor<Vec<u8>>, Metadata>::blank::<Metadata>()?
            .change_buffer(file)?;
            
        let mut page = blank.create_page("test")?;
        
        let millis = SystemTime::now().duration_since(UNIX_EPOCH).map_err(Error::other)?.as_millis();
        write!(&mut page, "{:?}", millis)?;
        let mut str = String::new();
        page.rewind()?;
        page.read_to_string(&mut str)?;
        println!("{}: {}", format!("{:?}", millis).len(), str.len());
        
        println!("{:#?}", &page.page_descriptor);
        
        // assert!(str.trim().len() == format!("{:?}", millis).len());
        
        Ok(())
    }
    
    #[test]
    pub fn read() -> Result<()> {
        let mut file = OpenOptions::new()
            .read(true)
            .open("/tmp/test.db")?;
            
        let mut db: crate::Database<File, Metadata> = crate::Database::open(file)?;
        
        assert!(db.leak_string_table().len() as u64 == db.string_table_range.length);
        
        Ok(())
    }
}
