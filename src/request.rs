use std::ops::{Deref, DerefMut};

use flume::{Receiver, Sender};
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};
use crate::storage::RemoteKey;
use crate::value::AnyValue;

#[derive(Debug)]
pub enum Callback {
    Resolve(Sender<Result<RemoteKey>>),
}

#[derive(Debug)]
pub enum CallbackValue {
    Resolve(Result<RemoteKey>),
}

impl From<Result<RemoteKey>> for CallbackValue {
    fn from(value: Result<RemoteKey>) -> Self {
        Self::Resolve(value)
    }
}

pub struct LocalResponse(Receiver<Result<AnyValue>>);

impl LocalResponse {
    pub async fn consume(self) -> Result<AnyValue> {
        self.0.recv_async().await?
    }
}

#[derive(Debug)]
pub enum Pending {
    Local(AnyValue),
}

#[derive(Debug)]
pub struct Processed<T>(T);

#[derive(Debug)]
pub struct Request<T> {
    reply: Sender<Result<AnyValue>>,
    state: T,
}

impl Request<Pending> {
    pub(crate) fn new(value: AnyValue) -> (Self, LocalResponse) {
        let (tx, rx) = flume::bounded(0);

        let req = Self {
            state: Pending::Local(value),
            reply: tx,
        };
        (req, LocalResponse(rx))
    }

    pub async fn reply(self, body: Result<AnyValue>) {
        // NOTE
        // there isn't much we can do here if the sender has closed down
        let _ = self.reply.send_async(body).await;
    }

    pub fn read<T: DeserializeOwned + 'static>(self) -> Result<Request<Processed<T>>> {
        let value = match self.state {
            Pending::Local(any_value) => match any_value.downcast::<T>() {
                Ok(val) => Ok(*val),
                Err(_) => Err(Error::InvalidValueType),
            },
            //ValueKind::Binary(data) => match self.state.1.deserialize(data) {
            //    Ok(val) => Ok(val),
            //    Err(_) => Err(Error::InvalidValueType),
            //},
        }?;

        let req = Request {
            reply: self.reply,
            state: Processed(value),
        };

        Ok(req)
    }
}

impl<T: Send + 'static> Request<Processed<T>> {
    pub async fn reply<R: Send + 'static>(self, value: R) -> Result<T> {
        let value = Box::new(value);
        self.reply.send_async(Ok(value)).await?;
        Ok(self.state.0)
    }

    pub fn take(self) -> (T, Request<Processed<()>>) {
        (
            self.state.0,
            Request {
                state: Processed(()),
                reply: self.reply,
            },
        )
    }
}

impl<T> Deref for Request<Processed<T>> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.state.0
    }
}

impl<T> DerefMut for Request<Processed<T>> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state.0
    }
}
