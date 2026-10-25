//! The HTTP server on 127.0.0.1: serves the page, routes `/api/*` to [`crate::api`],
//! counts connected browser tabs and exits the process once the last tab has been
//! gone for the linger period.
