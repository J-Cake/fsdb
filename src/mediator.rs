use std::io::{Read, Write, Seek};
use std::sync::Mutex;

use crate::error::Error;
use crate::format::Array;

pub enum RangeLock {
    Read(Array),
    Write(Array)
}

impl RangeLock {
    fn get_range(&self) -> Array {
        match self {
            Self::Read(range) => *range,
            Self::Write(range) => *range
        }
    }
}

pub(crate) struct Mediator<Backing> where Backing: Read + Write + Seek + 'static {
    locks: Mutex<Vec<RangeLock>>,
    backing: Mutex<Backing>
}

impl<Backing> Mediator<Backing> where Backing: Read + Write + Seek + 'static {
    pub fn try_read_range<Buffer>(&self, mut buffer: Buffer, offset: u64) -> Result<(), Error> where Buffer: AsMut<[u8]> {
        {
            let mut locks = self.locks.try_lock()?;
            if let None = locks.iter().find(|i| matches!(i, RangeLock::Write(range) if range.offset >= offset && range.end() < offset)) {
                locks.push(RangeLock::Read(Array {
                    offset,
                    length: buffer.as_mut().len() as u64,
                }));
            } else {
                return Err(Error::Busy);
            }
        }

        // I was hoping to avoid mutexes as they only allow a synchronised read/write operation.as
        // However, coordinating read/writes does exactly the same thing, and adds lots of code.
        // Plus the OS will synchronise read/writes across threads, so we ultimately gain nothing.
        self.backing.try_lock()?.read_exact(buffer.as_mut())?;

        Ok(())
    }

    pub fn try_write_range<Buffer>(&self, buffer: Buffer, offset: u64) -> Result<(), Error> where Buffer: AsRef<[u8]> {
        {
            let mut locks = self.locks.try_lock()?;
            if let None = locks.iter().find(|i| i.get_range().offset >= offset && i.get_range().end() < offset) {
                locks.push(RangeLock::Write(Array {
                    offset,
                    length: buffer.as_mut().len() as u64,
                }));
            } else {
                return Err(Error::Busy);
            }
        }

        // I was hoping to avoid mutexes as they only allow a synchronised read/write operation.as
        // However, coordinating read/writes does exactly the same thing, and adds lots of code.
        // Plus the OS will synchronise read/writes across threads, so we ultimately gain nothing.
        self.backing.try_lock()?.write_all(buffer.as_mut())?;

        Ok(())
    }
}
