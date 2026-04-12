use crate::types::{ArtifactId, ArtifactStatus, ArtifactType, NotebookId, SourceId};
use crate::errors::Result;
use async_trait::async_trait;

#[async_trait]
pub trait NotebookProvider: Send + Sync {
    // --- Notebooks ---
    async fn list_notebooks(&self) -> Result<Vec<(NotebookId, String)>>;
    async fn get_notebook(&self, notebook_id: &NotebookId) -> Result<serde_json::Value>;
    async fn create_notebook(&self, title: &str) -> Result<NotebookId>;
    async fn rename_notebook(&self, notebook_id: &NotebookId, new_title: &str) -> Result<()>;
    async fn delete_notebook(&self, notebook_id: &NotebookId) -> Result<()>;

    // --- Sources ---
    async fn list_sources(&self, notebook_id: &NotebookId) -> Result<Vec<SourceId>>;
    async fn add_source_text(&self, notebook_id: &NotebookId, title: &str, content: &str) -> Result<SourceId>;
    async fn delete_source(&self, notebook_id: &NotebookId, source_id: &SourceId) -> Result<()>;

    // --- Artefacts ---
    async fn list_artifacts(&self, notebook_id: &NotebookId) -> Result<Vec<(ArtifactId, String, ArtifactStatus)>>;
    async fn create_artifact(
        &self,
        notebook_id: &NotebookId,
        artifact_type: ArtifactType,
        source_ids: Option<Vec<SourceId>>,
    ) -> Result<ArtifactId>;
    async fn delete_artifact(&self, notebook_id: &NotebookId, artifact_id: &ArtifactId) -> Result<()>;
    
    /// Télécharge et extrait les données interactives pures d'un artefact terminé
    async fn fetch_interactive_data(&self, notebook_id: &NotebookId, artifact_id: &ArtifactId) -> Result<serde_json::Value>;
}
