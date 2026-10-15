//! OpenCollection storage and execution shared by the desktop and integrations.
pub mod bru;
mod codegen;
pub mod collection;
pub mod editing;
pub mod engine;
pub mod exchange;
pub mod mcp;
pub mod runner;
pub mod scripts;
pub mod settings;
pub mod testing;
pub mod update;
pub mod url_editor;
pub mod websocket;
pub use collection::{Collection, Document, DocumentKind};
pub use engine::{Event, Execution, RequestHead, Response, execute, prepare};
pub use settings::NetworkSettings;
