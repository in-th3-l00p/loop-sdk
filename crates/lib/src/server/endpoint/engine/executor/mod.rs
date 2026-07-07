mod native;

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::server::endpoint::{Handler, HandlerError, Source};
use crate::schema::Value;

pub enum Executor {
    Native(Arc<dyn Handler>),
    Stream(Arc<dyn Source>),
}

impl Executor {
    pub async fn call(&self, args: Vec<Value>) -> Result<Value, HandlerError> {
        match self {
            Executor::Native(handler) => native::call(handler.clone(), args).await,
            Executor::Stream(_) => {
                Err("streaming endpoint does not support request/response calls".into())
            }
        }
    }

    pub async fn stream(
        &self,
        args: Vec<Value>,
    ) -> Result<mpsc::Receiver<Result<Value, HandlerError>>, HandlerError> {
        match self {
            Executor::Stream(source) => native::subscribe(source.clone(), args).await,
            Executor::Native(handler) => {
                let result = native::call(handler.clone(), args).await;
                let (tx, rx) = mpsc::channel(1);
                let _ = tx.send(result).await;
                Ok(rx)
            }
        }
    }
}
