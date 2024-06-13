use std::{
   net::SocketAddr,
   path::{Path, PathBuf},
   time::Duration,
};

use axum::Router;
use log::info;
use notify::{RecursiveMode, Watcher};
use notify_debouncer_full::DebouncedEvent;
use tokio::{
   net::TcpListener,
   runtime::Runtime,
   signal,
   sync::{
      broadcast::{self, Sender},
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
   let router = Router::new().route_service("/*asset", serve_dir);

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
         .map_err(|reasons| Error::DebounceErrors(reasons))?
         .into_iter()
         .map(|DebouncedEvent { event, .. }| event.paths)
         .flatten()
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

   #[error("Watch error")]
   Watch { source: JoinError },

   #[error("Multiple server errors:\nwatch: {watch}\nserve: {serve}")]
   Compound { watch: JoinError, serve: JoinError },

   #[error("Building watcher\n{source}")]
   Watcher {
      #[from]
      source: notify::Error,
   },

   #[error(
      "Error in debounce server.\n{}",
      .0.iter()
         .map(|reason| format!("{reason}"))
         .collect::<Vec<_>>()
         .join("\n"))
   ]
   DebounceErrors(Vec<notify::Error>),
}
