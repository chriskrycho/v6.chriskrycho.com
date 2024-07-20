use std::{
   future::Future,
   net::SocketAddr,
   path::{Path, PathBuf},
   pin::pin,
   time::Duration,
};

use axum::{
   extract::{
      ws::{Message, WebSocket},
      State, WebSocketUpgrade,
   },
   response::Response,
   routing, Router,
};
use futures::{
   future::{self, Either},
   SinkExt, StreamExt,
};
use log::{debug, error, info};
use notify::{RecursiveMode, Watcher};
use notify_debouncer_full::DebouncedEvent;
use serde::Serialize;
use tokio::{
   net::TcpListener,
   runtime::Runtime,
   signal,
   sync::{
      broadcast::{self, error::RecvError, Sender},
      mpsc,
   },
   task::{self, JoinError},
};
use tower_http::services::ServeDir;
use watchexec::error::CriticalError;

// Initially, just rebuild everything. This can get smarter later!
use crate::build;

/// Serve the site, blocking on the result (i.e. blocking forever until it is
/// killed by some kind of signal or failure).
pub fn serve(path: &Path) -> Result<(), Error> {
   // Instead of making `main` be `async` (regardless of whether it needs it, as
   // many operations do *not*), make *this* function handle it. An alternative
   // would be to do this same basic wrapping in `main` but only for this.
   let rt = Runtime::new().map_err(|e| Error::Io { source: e })?;

   // TODO: need to (a) do this and (b) do re-builds when watch triggers it.
   // build_in(path).map_err(Error::from)?;

   // I only need the tx side, since we are going to take advantage of the fact that it
   // `broadcast::Sender` implements `Clone` to pass it around and get easy and convenient
   // access to local receivers with `tx.subscribe()`.
   let (tx, _rx) = broadcast::channel(10);

   let mut set = task::JoinSet::new();
   let server_handle = set.spawn_on(serve_in(path.to_owned(), tx.clone()), rt.handle());
   let watcher_handle = set.spawn_on(watch_in(path.to_owned(), tx.clone()), rt.handle());

   set.spawn_on(
      async move {
         signal::ctrl_c()
            .await
            .map_err(|source| Error::Io { source })?;
         server_handle.abort();
         watcher_handle.abort();
         Ok(())
      },
      rt.handle(),
   );

   rt.block_on(async {
      while let Some(result) = set.join_next().await {
         match result {
            Ok(Ok(_)) => {
               // ignore it and keep waiting for the rest to complete
               // in the future, trace it
               // maybe: if one of them *completes* doesn’t that mean we should shut down?
            }
            Ok(Err(reason)) => return Err(reason),
            Err(join_error) => return Err(Error::Serve { source: join_error }),
         }
      }

      Ok(())
   })
}

async fn serve_in(path: PathBuf, state: Tx) -> Result<(), Error> {
   // This could be extracted into its own function.
   let serve_dir = ServeDir::new(path).append_index_html_on_directories(true);
   let router = Router::new()
      .route_service("/*asset", serve_dir)
      .route("/live-reload", routing::get(websocket_upgrade))
      .with_state(state);

   let addr = SocketAddr::from(([127, 0, 0, 1], 24747)); // 24747 = CHRIS on a phone 🤣
   let listener = TcpListener::bind(addr)
      .await
      .map_err(|e| Error::BadAddress {
         value: addr,
         source: e,
      })?;

   info!("→ Serving at: http://{addr}");

   axum::serve(listener, router)
      .await
      .map_err(|source| Error::ServeStart { source })
}

async fn websocket_upgrade(
   extractor: WebSocketUpgrade,
   State(state): State<Tx>,
) -> Response {
   debug!("binding websocket upgrade");
   extractor.on_upgrade(|socket| {
      debug!("upgrading the websocket");
      websocket(socket, state)
   })
}

async fn websocket(socket: WebSocket, state: Sender<Change>) {
   let (mut ws_tx, mut ws_rx) = socket.split();
   let mut change_rx = state.subscribe();

   let reload = pin!(async {
      loop {
         match change_rx.recv().await {
            Ok(Change { paths }) => {
               let paths_desc = paths
                  .iter()
                  .map(|p| p.to_string_lossy())
                  .collect::<Vec<_>>()
                  .join("\n\t");
               debug!("sending WebSocket reload message with paths:\n\t{paths_desc}");

               let payload = serde_json::to_string(&ChangePayload::Reload { paths })
                  .unwrap_or_else(|e| panic!("Could not serialize payload: {e}"));

               match ws_tx.send(Message::Text(payload)).await {
                  Ok(_) => debug!("Successfully sent {paths_desc}"),
                  Err(reason) => error!("Could not send WebSocket message:\n{reason}"),
               }
            }

            Err(recv_error) => match recv_error {
               RecvError::Closed => break,
               RecvError::Lagged(skipped) => {
                  error!("Websocket change notifier: lost {skipped} messages");
               }
            },
         }
      }
   });

   let close = pin!(async {
      while let Some(message) = ws_rx.next().await {
         match handle(message) {
            Ok(state) => debug!("{state}"),

            Err(error) => {
               debug!("WebSocket error:\n{error}");
               break;
            }
         }
      }
   });

   (reload, close).race().await;
}

fn handle(message_result: Result<Message, axum::Error>) -> Result<WebSocketState, Error> {
   debug!("Received {message_result:?} from WebSocket.");

   use Message::*;
   match message_result {
      Ok(message) => match message {
         Text(content) => Err(Error::UnexpectedString(content)),

         Binary(content) => Err(Error::UnexpectedBytes(content)),

         Ping(bytes) => {
            debug!("Ping with bytes: {bytes:?}");
            Ok(WebSocketState::Open)
         }

         Pong(bytes) => {
            debug!("Ping with bytes: {bytes:?}");
            Ok(WebSocketState::Open)
         }

         Close(maybe_frame) => {
            let message = WebSocketState::Closed {
               reason: maybe_frame.map(|frame| {
                  let desc = if !frame.reason.is_empty() {
                     format!("Reason: {};", frame.reason)
                  } else {
                     String::from("")
                  };

                  let code = format!("Code: {}", frame.code);
                  desc + &code
               }),
            };

            Ok(message)
         }
      },

      Err(source) => Err(Error::WebsocketReceive { source }),
   }
}

#[derive(Debug, Serialize)]
enum ChangePayload {
   Reload { paths: Vec<PathBuf> },
}

#[derive(Debug)]
enum WebSocketState {
   Open,
   Closed { reason: Option<String> },
}

impl std::fmt::Display for WebSocketState {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      use WebSocketState::*;
      match self {
         Open => write!(f, "WebSocket state: open"),
         Closed {
            reason: Some(reason),
         } => write!(f, "WebSocket state: closed. Cause:\n{reason}"),
         Closed { reason: None } => write!(f, "WebSocket state: closed."),
      }
   }
}

#[derive(Debug, Clone)]
struct Change {
   pub paths: Vec<PathBuf>,
}

/// Shorthand for typing!
type Tx = Sender<Change>;

async fn watch_in(dir: PathBuf, change_tx: Tx) -> Result<(), Error> {
   let (tx, mut rx) = mpsc::channel(256);

   // Doing this here means we will not drop the watcher until this function
   // ends, and the `while let` below will continue until there is an error (or
   // something else shuts down the whole system here!).
   let mut debouncer = notify_debouncer_full::new_debouncer(
      Duration::from_secs(1),
      /*tick_rate */ None,
      move |result| {
         if let Err(e) = tx.blocking_send(result) {
            eprintln!("Could not send event.\nError:{e}");
         }
      },
   )?;

   let watcher = debouncer.watcher();
   watcher.watch(&dir, RecursiveMode::Recursive)?;

   while let Some(result) = rx.recv().await {
      let paths = result
         .map_err(Error::DebounceErrors)?
         .into_iter()
         .flat_map(|DebouncedEvent { event, .. }| event.paths)
         .collect::<Vec<_>>();

      let change = Change { paths };
      if let Err(e) = change_tx.send(change) {
         eprintln!("Error sending out: {e:?}");
      }
   }

   Ok(())
}

#[derive(Debug, thiserror::Error)]
#[error("Error serving site")]
pub enum Error {
   #[error("Build error:\n{source}")]
   Build {
      #[from]
      source: build::Error,
   },

   #[error("I/O error\n{source}")]
   Io { source: std::io::Error },

   #[error("Error starting file watcher\n{source}")]
   WatchStart {
      #[from]
      source: CriticalError,
   },

   #[error("Could not open socket on address: {value}\n{source}")]
   BadAddress {
      value: SocketAddr,
      source: std::io::Error,
   },

   #[error("Could not start the site server\n{source}")]
   ServeStart { source: std::io::Error },

   #[error("Error while serving the site\n{source}")]
   Serve { source: JoinError },

   #[error("Runtime error\n{source}")]
   Tokio {
      #[from]
      source: JoinError,
   },

   #[error("Building watcher\n{source}")]
   Watcher {
      #[from]
      source: notify::Error,
   },

   #[error(
      "Debouncing changes from the file system:\n{}",
      .0.iter()
         .map(|reason| format!("{reason}"))
         .collect::<Vec<_>>()
         .join("\n"))
   ]
   DebounceErrors(Vec<notify::Error>),

   #[error("Could not receive WebSocket message:\n{source}")]
   WebsocketReceive { source: axum::Error },

   #[error("Unexpectedly received string WebSocket message with content:\n{0}")]
   UnexpectedString(String),

   #[error("Unexpectedly received binary WebSocket message with bytes:\n{0:?}")]
   UnexpectedBytes(Vec<u8>),
   // TODO: use this when handling errors without panicking in WebSocket handler
   // #[error("Could not serialize data:\n{source}")]
   // Serialize { source: serde_json::Error },
}

trait Race<T, U>: Sized {
   async fn race(self) -> Either<T, U>;
}

impl<A, B, F1, F2> Race<A, B> for (F1, F2)
where
   A: Sized,
   B: Sized,
   F1: Future<Output = A> + Unpin,
   F2: Future<Output = B> + Unpin,
{
   async fn race(self) -> Either<A, B> {
      race(self.0, self.1).await
   }
}

async fn race<A, B, F1, F2>(f1: F1, f2: F2) -> Either<A, B>
where
   F1: Future<Output = A> + Unpin,
   F2: Future<Output = B> + Unpin,
{
   match future::select(f1, f2).await {
      Either::Left((a, _f2)) => Either::Left(a),
      Either::Right((b, _f1)) => Either::Right(b),
   }
}
