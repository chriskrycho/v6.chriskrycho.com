use std::{net::SocketAddr, path::PathBuf};

use axum::{routing::get, Router};
use log::info;
use tokio::{net::TcpListener, runtime::Runtime};
use tower_http::services::ServeDir;

// Initially, just rebuild everything. This can get smarter later!
use crate::build::{self, build_in};

/// Serve the site, blocking on the result (i.e. blocking forever until it is
/// killed by some kind of signal or failure).
pub fn serve(path: &PathBuf) -> Result<(), Error> {
   // Instead of making `main` be `async` (regardless of whether it needs it, as
   // many operations do *not*), make *this* function handle it. An alternative
   // would be to do this same basic wrapping in `main` but only for this.
   let rt = Runtime::new().map_err(Error::from)?;
   rt.block_on(async {
      let serve_dir = ServeDir::new(path).append_index_html_on_directories(true);
      let router = Router::new().route_service("/*asset", serve_dir);

      let addr = SocketAddr::from(([127, 0, 0, 1], 9876));
      let listener = TcpListener::bind(addr).await.unwrap();
      info!("→ Serving at: http://{addr}");
      axum::serve(listener, router).await.unwrap();
   });

   build_in(path).map_err(Error::from)
}

#[derive(Debug, thiserror::Error)]
#[error("Error serving site")]
pub enum Error {
   #[error("Build error: {0}")]
   Build(#[from] build::Error),

   #[error("I/O error: {0}")]
   Io(#[from] std::io::Error),
}
