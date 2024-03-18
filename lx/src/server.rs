use std::{
   net::SocketAddr,
   path::{Path, PathBuf},
   sync::Arc,
   time::Duration,
};

use axum::Router;
use log::info;
use tokio::{join, net::TcpListener, runtime::Runtime, select, task::JoinError};
use tower_http::services::ServeDir;
use watchexec::{error::CriticalError, sources::fs::WatchedPath, Watchexec};
use watchexec_signals::Signal;

// Initially, just rebuild everything. This can get smarter later!
use crate::build::{self, build_in};

/// Serve the site, blocking on the result (i.e. blocking forever until it is
/// killed by some kind of signal or failure).
pub fn serve(path: &Path) -> Result<(), Error> {
   // TODO: need to (a) do this and (b) do re-builds when watch triggers it.
   // build_in(path).map_err(Error::from)?;

   // Instead of making `main` be `async` (regardless of whether it needs it, as
   // many operations do *not*), make *this* function handle it. An alternative
   // would be to do this same basic wrapping in `main` but only for this.
   let rt = Runtime::new().map_err(|e| Error::Io { source: e })?;

   let watch_path = path.to_owned();

   let watch = rt.spawn(async move {
      let watcher = watcher_in(watch_path)?;
      watcher
         .main()
         .await
         .map_err(Error::from)
         .and_then(|inner| inner.map_err(Error::from))
   });

   // This is stupid. 😆
   match rt.block_on(async { join!(watch, serve_in(path, &rt)) }) {
      (Ok(_), Ok(_)) => Ok(()),
      (Ok(_), Err(serve_err)) => Err(Error::Serve { source: serve_err }),
      (Err(watch_err), Ok(_)) => Err(Error::Watch { source: watch_err }),
      (Err(watch_err), Err(serve_err)) => Err(Error::Compound {
         watch: watch_err,
         serve: serve_err,
      }),
   }
}

fn serve_in(path: &Path, rt: &Runtime) -> tokio::task::JoinHandle<Result<(), Error>> {
   // This could be extracted into its own function.
   let serve_dir = ServeDir::new(path).append_index_html_on_directories(true);
   let router = Router::new().route_service("/*asset", serve_dir);
   let serve = rt.spawn(async {
      let addr = SocketAddr::from(([127, 0, 0, 1], 9876));
      let listener = TcpListener::bind(addr)
         .await
         .map_err(|e| Error::BadAddress {
            value: addr,
            source: e,
         })?;

      info!("→ Serving at: http://{addr}");

      axum::serve(listener, router)
         .await
         .map_err(|e| Error::ServeStart { source: e })
   });
   serve
}

fn watcher_in(path: PathBuf) -> Result<Arc<Watchexec>, Error> {
   let watcher = Watchexec::new(|mut handler| {
      // This needs `.iter()` because `events` is an `Arc<[Event]>`, not just
      // `[Event]`, so `.iter()` delegates to the inner bit.
      for event in handler.events.iter() {
         info!("Event: {event:#?}");
      }

      // TODO: this needs to send a “please shut it all down” signal out of the
      // async handler. As is, this may be fine once properly composed with
      // another handler, e.g. via `join!`.
      if handler.signals().any(|sig| sig == Signal::Interrupt) {
         handler.quit_gracefully(Signal::Interrupt, Duration::from_secs(1));
      }

      handler
   })
   .map_err(Error::from)?;

   watcher.config.pathset([path]);
   Ok(watcher)
}

#[derive(Debug, thiserror::Error)]
#[error("Error serving site")]
pub enum Error {
   #[error("Build error:\n{source}")]
   Build {
      #[from]
      source: build::Error,
   },

   #[error("I/O error")]
   Io { source: std::io::Error },

   #[error("Error starting file watcher")]
   WatchStart {
      #[from]
      source: CriticalError,
   },

   #[error("Could not open socket on address: {value}")]
   BadAddress {
      value: SocketAddr,
      source: std::io::Error,
   },

   #[error("Could not start the site server")]
   ServeStart { source: std::io::Error },

   #[error("Error while serving the site")]
   Serve { source: JoinError },

   #[error("Runtime error")]
   Tokio {
      #[from]
      source: JoinError,
   },

   #[error("Watch error")]
   Watch { source: JoinError },

   #[error("Multiple server errors:\nwatch: {watch}\nserve: {serve}")]
   Compound { watch: JoinError, serve: JoinError },
}
