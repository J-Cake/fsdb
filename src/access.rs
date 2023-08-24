/// Stores access information - this structure does no enforcement of access of any sorts. It is up to the caller to interpret and check this.
#[derive(Debug, Clone)]
pub enum Access {
    None(String),
    Read(String),
    ReadWrite(String),
    ReadWriteExecute(String),
    ReadExecute(String),
    Custom(String, u8)
}
