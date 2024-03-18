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
         .map_err(|e| Error::Serve { source: e })
   });

   // This is stupid. 😆
   let (watch_result, serve_result) = rt.block_on(async { join!(watch, serve) });
   let _ = watch_result??;
   let _ = serve_result??;

   Ok(())
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

   #[error("Watch error")]
   Watch {
      #[from]
      source: CriticalError,
   },

   #[error("Could not open socket on address: {value}")]
   BadAddress {
      value: SocketAddr,
      source: std::io::Error,
   },

   #[error("Could not serve the site")]
   Serve { source: std::io::Error },

   #[error("Runtime error")]
   Tokio {
      #[from]
      source: JoinError,
   },
}
