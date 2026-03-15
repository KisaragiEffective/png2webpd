use std::error::Error;
use crate::convert::ConvertedRecord;

pub enum Command<T> {
    Shutdown { graceful: bool },
    Message(T),
}

pub enum ConsoleMessage {
    Error {
        e: Box<dyn Error + Send + Sync>
    },
    Processed {
        record: ConvertedRecord
    }
}