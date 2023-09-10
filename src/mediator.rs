use std::io::Error;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Result;
use std::io::Write;
use std::io::Seek;
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::Arc;
use std::sync::MutexGuard;

pub(crate) type MutexChunk<Backing> = Arc<Mutex<Chunk<Backing>>>;

/// The mediator is responsible for performing read/writes on the underlying buffer. 
/// The `Database` struct offers the necessary infrastructure to convert this into useful information, as well as parsing the underlying structure.
pub(crate) struct Mediator<Backing>
where 
    Backing: Read + Write + Seek 
{
    backing: Backing,
    locks: Arc<Mutex<BTreeMap<crate::Array, Arc<Mutex<Chunk<Backing>>>>>>,
    seek_offset: usize
}

impl<Backing> Mediator<Backing>
where
    Backing: Read + Write + Seek 
{
    pub fn get_chunk(&mut self, array: crate::Array) -> Result<MutexChunk<Backing>> {
        let locks: MutexGuard<BTreeMap<crate::Array, MutexChunk<Backing>>> = Arc::clone(&self.locks)
            .lock()
            .map_err(|| Error::new(ErrorKind::ResourceBusy, array.offset))?;
    
        Ok(if let Some(chunk) = locks.get(&array) {
            Arc::clone(&chunk)
        } else {
            let chunk = Arc::new(Mutex::new(Chunk {
                bounds: array,
                buffer: Vec::with_capacity(array.length)
            }));
            
            locks.insert(array, Arc::clone(&chunk));
            Arc::clone(&chunk)
        })
    }
    
    pub fn try_flush(&mut self) -> Result<()> {
        let locks: MutexGuard<BTreeMap<crate::Array, MutexChunk<Backing>>> = Arc::clone(&self.locks)
            .try_lock()
            .map_err(|| Error::new(ErrorKind::ResourceBusy, format!("Flush failed")))?;
            
        for (a, i) in locks.iter() {
            let chunk: MutexGuard<Chunk<Backing>> = Arc::clone(&i)
                .try_lock()
                .map_err(|| Error::new(ErrorKind::ResourceBusy, format!("Range Busy: {}:{}", a.offset, a.offset + a.length)))?;
                
            self.backing.seek(a.offset)?;
            self.backing.write_all(&chunk.buffer)?;
        }
            
        Ok(())
    }
}

pub(crate) struct Chunk<Backing>
where
    Backing: Read + Write + Seek
{
    pub bounds: crate::Array,
    pub(crate) buffer: Vec<u8>
}

impl<Backing> Read for Chunk<Backing>
where
    Backing: Read + Write + Seek 
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        buf.copy_from_slice(&self.buffer[0..buf.len()]);
        
        Ok(buf.len())
    }
}

impl<Backing> Write for Chunk<Backing>
where
    Backing: Read + Write + Seek 
{
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.buffer.copy_from_slice(&buf);
        Ok(buf.len())
    }
}


