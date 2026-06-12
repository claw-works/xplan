use futures_lite::StreamExt;
use tokio::sync::broadcast;

const CHANNEL: &str = "xplan:cache:invalidate";

#[derive(Debug, Clone)]
pub enum InvalidationEvent {
    Routes,
    ClientKey(String),
    Prices,
    All,
}

pub struct InvalidationPublisher {
    pool: deadpool_redis::Pool,
}

impl InvalidationPublisher {
    pub fn new(pool: deadpool_redis::Pool) -> Self {
        Self { pool }
    }

    pub async fn publish(&self, event: &InvalidationEvent) -> anyhow::Result<()> {
        let msg = match event {
            InvalidationEvent::Routes => "routes".to_string(),
            InvalidationEvent::ClientKey(hash) => format!("client_key:{}", hash),
            InvalidationEvent::Prices => "prices".to_string(),
            InvalidationEvent::All => "all".to_string(),
        };
        let mut conn = self.pool.get().await?;
        redis::cmd("PUBLISH")
            .arg(CHANNEL)
            .arg(&msg)
            .query_async::<()>(&mut *conn)
            .await?;
        Ok(())
    }
}

pub fn spawn_invalidation_listener(
    redis_url: String,
    tx: broadcast::Sender<InvalidationEvent>,
) {
    tokio::spawn(async move {
        loop {
            match listen_loop(&redis_url, &tx).await {
                Ok(()) => break,
                Err(e) => {
                    tracing::error!("Cache invalidation listener error: {}, reconnecting...", e);
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    });
}

async fn listen_loop(
    redis_url: &str,
    tx: &broadcast::Sender<InvalidationEvent>,
) -> anyhow::Result<()> {
    let client = redis::Client::open(redis_url)?;
    let mut pubsub = client.get_async_pubsub().await?;
    pubsub.subscribe(CHANNEL).await?;

    let mut stream = pubsub.into_on_message();
    loop {
        let msg = stream.next().await;
        if let Some(msg) = msg {
            let payload: String = msg.get_payload()?;
            let event = parse_event(&payload);
            let _ = tx.send(event);
        } else {
            break;
        }
    }
    Ok(())
}

fn parse_event(payload: &str) -> InvalidationEvent {
    if payload == "routes" {
        InvalidationEvent::Routes
    } else if payload == "prices" {
        InvalidationEvent::Prices
    } else if payload == "all" {
        InvalidationEvent::All
    } else if let Some(hash) = payload.strip_prefix("client_key:") {
        InvalidationEvent::ClientKey(hash.to_string())
    } else {
        InvalidationEvent::All
    }
}
