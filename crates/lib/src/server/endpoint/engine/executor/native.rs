use std::sync::Arc;

use tokio::sync::mpsc;

use crate::server::endpoint::{Handler, HandlerError, Source};
use crate::schema::Value;

pub async fn call(handler: Arc<dyn Handler>, args: Vec<Value>) -> Result<Value, HandlerError> {
    tokio::task::spawn_blocking(move || handler.call(&args))
        .await
        .map_err(|e| -> HandlerError { format!("handler panicked: {e}").into() })?
}

pub async fn subscribe(
    source: Arc<dyn Source>,
    args: Vec<Value>,
) -> Result<mpsc::Receiver<Result<Value, HandlerError>>, HandlerError> {
    let stream = tokio::task::spawn_blocking(move || source.subscribe(&args))
        .await
        .map_err(|e| -> HandlerError { format!("source panicked: {e}").into() })??;

    let (tx, rx) = mpsc::channel(16);
    tokio::task::spawn_blocking(move || {
        for item in stream {
            if tx.blocking_send(item).is_err() {
                break;
            }
        }
    });

    Ok(rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn call_runs_handler_off_the_async_thread() {
        let handler: Arc<dyn Handler> = Arc::new(|args: &[Value]| match args {
            [Value::I64(n)] => Ok(Value::I64(n * 2)),
            _ => Err("bad args".into()),
        });

        assert_eq!(
            call(handler, vec![Value::I64(21)]).await.unwrap(),
            Value::I64(42)
        );
    }

    #[tokio::test]
    async fn call_surfaces_handler_panics_as_errors() {
        let handler: Arc<dyn Handler> =
            Arc::new(|_: &[Value]| -> Result<Value, HandlerError> { panic!("kaboom") });

        assert!(call(handler, vec![]).await.is_err());
    }

    #[tokio::test]
    async fn subscribe_bridges_iterator_to_channel_in_order() {
        let source: Arc<dyn Source> = Arc::new(
            |_: &[Value]| -> Result<crate::server::endpoint::ValueStream, HandlerError> {
                Ok(Box::new((0..3).map(|i| Ok(Value::I64(i)))))
            },
        );

        let mut rx = subscribe(source, vec![]).await.unwrap();
        let mut received = Vec::new();
        while let Some(item) = rx.recv().await {
            received.push(item.unwrap());
        }

        assert_eq!(received, vec![Value::I64(0), Value::I64(1), Value::I64(2)]);
    }

    #[tokio::test]
    async fn subscribe_propagates_mid_stream_errors() {
        let source: Arc<dyn Source> = Arc::new(
            |_: &[Value]| -> Result<crate::server::endpoint::ValueStream, HandlerError> {
                Ok(Box::new(
                    vec![Ok(Value::I64(1)), Err::<Value, HandlerError>("lost".into())].into_iter(),
                ))
            },
        );

        let mut rx = subscribe(source, vec![]).await.unwrap();
        assert_eq!(rx.recv().await.unwrap().unwrap(), Value::I64(1));
        assert!(rx.recv().await.unwrap().is_err());
    }
}
