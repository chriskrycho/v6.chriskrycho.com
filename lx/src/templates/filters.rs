use log::trace;
use minijinja::{Environment, State};

pub fn add_filters(env: &mut Environment) {
   env.add_filter("page_title", page_title);
}

fn page_title(state: &State, page_title: &str) -> String {
   let base = "Sympolymathesy, by Chris Krycho";
   let current_page = state.name();
   let value = if current_page == "index.html" {
      base.into()
   } else {
      format!("{page_title} – {base}")
   };
   trace!("rendering page title {page_title} in template {current_page}: {value}");
   value
}
