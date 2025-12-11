pub mod form_data;
pub mod static_files;
pub mod websocket;

pub use form_data::handle_form_data;
pub use static_files::handle_static_file;
pub use websocket::handle_websocket_upgrade;
