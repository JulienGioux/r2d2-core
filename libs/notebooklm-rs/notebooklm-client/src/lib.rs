pub mod actor;
pub mod browser_adapter;
pub mod endpoints;

// Re-export the primary client interface
pub use browser_adapter::NotebookClient;
