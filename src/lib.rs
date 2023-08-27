#![feature(io_error_other)]
#![feature(write_all_vectored)]

pub mod database;
pub mod page;
pub mod access;

pub use database::*;
pub use page::*;
pub use access::*;

#[cfg(test)]
pub mod test {
    use std::fs::File;
    use std::fs::OpenOptions;
    use std::io::Result;
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
    
//     #[test]
//     pub fn blank() -> Result<()> {
//         let mut file = OpenOptions::new()
//             .create(true)
//             .read(true)
//             .write(true)
//             .open("/tmp/test.db")?;
//             
//         let mut blank = crate::Database::<Cursor<Vec<u8>>, Metadata>::blank::<Metadata>()?
//             .change_buffer(file)?;
//             
//         Ok(())
//     }
//     
//     #[test]
//     pub fn create_page() -> Result<()> {
//         let mut file = OpenOptions::new()
//             .create(true)
//             .read(true)
//             .write(true)
//             .open("/tmp/test.db")?;
//         
//         let mut blank = crate::Database::<Cursor<Vec<u8>>, Metadata>::blank::<Metadata>()?
//             .change_buffer(file)?;
//             
//         let page = blank.create_page("test")?;
//         
//         Ok(())
//     }
//     
//     #[test]
//     pub fn read_write() -> Result<()> {
//         let mut file = OpenOptions::new()
//             .create(true)
//             .read(true)
//             .write(true)
//             .open("/tmp/test.db")?;
//         
//         let mut blank = crate::Database::<Cursor<Vec<u8>>, Metadata>::blank::<Metadata>()?
//             .change_buffer(file)?;
//             
//         let page = blank.create_page("test")?;
//         
//         
//         
//         Ok(())
//     }
//     
    #[test]
    pub fn read() -> Result<()> {
        let mut file = OpenOptions::new()
            .read(true)
            .open("/home/jcake/Erika/Frameworks/datastore-provider/test/header.db")?;
            
        let mut db: crate::Database<File, Metadata> = crate::Database::open(file)?;
        
        assert!(db.leak_string_table().len() as u64 == db.string_table_range.length);
        
        Ok(())
    }
}
