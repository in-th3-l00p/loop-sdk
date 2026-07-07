use std::sync::Arc;

use tokio::sync::mpsc;

use crate::server::endpoint::{Context, Handler, HandlerError, Source};
use crate::schema::Value;

pub async fn call(
    handler: Arc<dyn Handler>,
    ctx: Context,
    args: Vec<Value>,
) -> Result<Value, HandlerError> {
    tokio::task::spawn_blocking(move || handler.call(&ctx, &args))
        .await
        .map_err(|e| -> HandlerError { format!("handler panicked: {e}").into() })?
}

pub async fn subscribe(
    source: Arc<dyn Source>,
    ctx: Context,
    args: Vec<Value>,
) -> Result<mpsc::Receiver<Result<Value, HandlerError>>, HandlerError> {
    let stream = tokio::task::spawn_blocking(move || source.subscribe(&ctx, &args))
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
        let handler: Arc<dyn Handler> = Arc::new(|_: &Context, args: &[Value]| match args {
            [Value::I64(n)] => Ok(Value::I64(n * 2)),
            _ => Err("bad args".into()),
        });

        assert_eq!(
            call(handler, Context::default(), vec![Value::I64(21)])
                .await
                .unwrap(),
            Value::I64(42)
        );
    }

    #[tokio::test]
    async fn call_surfaces_handler_panics_as_errors() {
        let handler: Arc<dyn Handler> = Arc::new(
            |_: &Context, _: &[Value]| -> Result<Value, HandlerError> { panic!("kaboom") },
        );

        assert!(call(handler, Context::default(), vec![]).await.is_err());
    }

    #[tokio::test]
    async fn call_delivers_the_request_context() {
        let handler: Arc<dyn Handler> = Arc::new(|ctx: &Context, _: &[Value]| {
            Ok(Value::Str(ctx.token().unwrap_or("none").to_string()))
        });

        assert_eq!(
            call(handler, Context::with_token("t0"), vec![])
                .await
                .unwrap(),
            Value::Str("t0".into())
        );
    }

    #[tokio::test]
    async fn subscribe_bridges_iterator_to_channel_in_order() {
        let source: Arc<dyn Source> = Arc::new(
            |_: &Context, _: &[Value]| -> Result<crate::server::endpoint::ValueStream, HandlerError> {
                Ok(Box::new((0..3).map(|i| Ok(Value::I64(i)))))
            },
        );

        let mut rx = subscribe(source, Context::default(), vec![]).await.unwrap();
        let mut received = Vec::new();
        while let Some(item) = rx.recv().await {
            received.push(item.unwrap());
        }

        assert_eq!(received, vec![Value::I64(0), Value::I64(1), Value::I64(2)]);
    }

    #[tokio::test]
    async fn subscribe_propagates_mid_stream_errors() {
        let source: Arc<dyn Source> = Arc::new(
            |_: &Context, _: &[Value]| -> Result<crate::server::endpoint::ValueStream, HandlerError> {
                Ok(Box::new(
                    vec![Ok(Value::I64(1)), Err::<Value, HandlerError>("lost".into())].into_iter(),
                ))
            },
        );

        let mut rx = subscribe(source, Context::default(), vec![]).await.unwrap();
        assert_eq!(rx.recv().await.unwrap().unwrap(), Value::I64(1));
        assert!(rx.recv().await.unwrap().is_err());
    }
}
