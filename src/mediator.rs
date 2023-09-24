use std::io::Read;
use std::io::Seek;
use std::io::Write;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::thread::JoinHandle;

use crate::Array;
use crate::Error;

enum RequestAction {
    Read,
    Write,
    Unload,
}

enum RequestStatus {
    Pending,
    Success,
    Failed(Error),
}

impl RequestAction {
    pub fn req(self, range: Array) -> Request {
        Request {
            action: self,
            cvar: Arc::new((
                Mutex::new((Vec::new(), RequestStatus::Pending)),
                Condvar::new(),
            )),
            range,
        }
    }
}

struct Request {
    action: RequestAction,
    cvar: Arc<(Mutex<(Vec<u8>, RequestStatus)>, Condvar)>,
    range: Array,
}

pub(crate) struct Mediator {
    join_handle: JoinHandle<()>,
}

impl Mediator {
    pub(crate) fn new<Backing>(backing: Backing) -> Result<Arc<Self>, Error>
    where
        Backing: Read + Write + Seek + Send,
    {
        let (send_request, receive_request) = channel::<Request>();

        Ok(Self {
            join_handle: std::thread::spawn(move || -> Result<(), Error> {
                let mut cache = Vec::<Box<(Array, [u8])>>::new();
                let mut backing = backing;

                // Continuously process incoming requests.
                while let Ok(req) = receive_request.recv() {
                    let cvar = Arc::clone(&req.cvar);
                    let (mut buffer, mut status) = cvar.0.lock()?;

                    let index = match cache.binary_search_by(|i| i.0.offset.cmp(*req.range.offset))
                    {
                        Ok(index) => index,
                        Err(index) => index,
                    };

                    *status = if let Some(cache_entry) = cache.get(index) {
                        if cache_entry.0.offset <= req.range.offset + req.range.length {
                            RequestStatus::Failed(Error::Busy)
                        }
                    } else {                        
                        let result = if let Err(err) = match req.action {
                            RequestAction::Read => backing.read_exact(&mut buffer),
                            RequestAction::Write => backing.write_all(&buffer),
                            RequestAction::Unload => todo!(),
                        } {
                            RequestStatus::Failed(Error::Other("Failed to handle request"))
                        } else {
                            RequestStatus::Success
                        };
                        
                        cache.insert(index, Box::new((req.range.clone(), buffer.clone())));
                        result
                    };
                    
                    cvar.1.notify_all();
                }

                Ok(())
            }),
        })
    }

    pub fn get(&self, array: Array) -> Result<&[u8], Error> {
        todo!();
    }

    pub fn get_mut(&mut self, array: Array) -> Result<&mut [u8], Error> {
        todo!();
    }
}
