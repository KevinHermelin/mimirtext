use crate::{
    graph::RepositoryGraph,
    repository::{NoteKey, Repository},
};
use core::{error, fmt};
use std::{
    io,
    sync::{Arc, RwLock, mpsc::Sender},
    thread,
};

#[derive(Debug)]
enum GraphThreadError {
    RepositoryList(String, io::Error),
    NoteParse(NoteKey, io::Error),
}

impl fmt::Display for GraphThreadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::RepositoryList(repo_id, ..) => {
                write!(f, "could not list notes from repository {:?}", repo_id)
            }
            Self::NoteParse(note_key, ..) => {
                write!(f, "could not parse note {:?}", note_key)
            }
        }
    }
}

impl error::Error for GraphThreadError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::RepositoryList(_, e) => Some(e),
            Self::NoteParse(_, e) => Some(e),
        }
    }
}

/// Starts building a graph in a separate thread.
///
/// Note that this will fail silently.
pub fn create_graph_builder(
    repository: Arc<RwLock<dyn Repository + Send + Sync>>,
    graph: Arc<RwLock<RepositoryGraph>>,
    graph_progress_tx: Sender<GraphBuildProgress>,
) {
    thread::spawn(move || {
        // TODO: Detect if thread fails.
        build_graph(repository, graph, graph_progress_tx);
    });
}

/// Represents the progress of a graph building job.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum GraphBuildProgress {
    /// Graph building has not started or is finished.
    #[default]
    Idle,

    /// Graph building is still ongoing.
    InProgress(f32),
}

impl GraphBuildProgress {
    /// Creates a state from item count.
    ///
    /// It will be InProgress as long as done != total.
    /// Otherwise, this returns Idle.
    fn from_count(done: usize, total: usize) -> Self {
        if done == total {
            return Self::Idle;
        }

        let done = done as f32;
        let total = total as f32;
        Self::InProgress(done / total)
    }
}

fn build_graph(
    repository: Arc<RwLock<dyn Repository>>,
    graph: Arc<RwLock<RepositoryGraph>>,
    graph_progress_tx: Sender<GraphBuildProgress>,
) -> Result<(), GraphThreadError> {
    // Important that repository is not locked for later.
    let repo_id = repository.read().unwrap().id().to_string();
    let queue = repository
        .read()
        .unwrap()
        .notes()
        .map_err(|error| GraphThreadError::RepositoryList(repo_id, error))?
        .clone();

    let notes_count = queue.len();

    for (i, key) in queue.iter().enumerate() {
        let NoteKey(_, id) = key.clone();

        let note = repository
            .read()
            .unwrap()
            .note(&id)
            .map_err(|error| GraphThreadError::NoteParse(key.clone(), error));

        // Silently ignoring all files which cannot be read. Otherwise, this would panic on pictures in the repo.
        // TODO: This should be better handled.
        if let Ok(note) = note {
            graph.write().unwrap().register_note(&note);
        }

        graph_progress_tx
            .send(GraphBuildProgress::from_count(i + 1, notes_count))
            .expect("should be able to send graph progress");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::mock::MockRepository;
    use std::sync::{Arc, mpsc};

    #[test]
    fn test_graph_build() {
        let mut repo = MockRepository::new();
        let note_a = repo.insert_note("note a", "");
        let note_b = repo.insert_note("note b", "");
        let note_c = repo.insert_note("note c", "");

        let graph = Arc::new(RwLock::new(RepositoryGraph::new()));

        let (graph_progress_tx, graph_progress_rx) = mpsc::channel();
        create_graph_builder(
            Arc::new(RwLock::new(repo)),
            Arc::clone(&graph),
            graph_progress_tx,
        );

        assert!(
            graph_progress_rx.iter().eq([
                GraphBuildProgress::InProgress(1.0 / 3.0),
                GraphBuildProgress::InProgress(2.0 / 3.0),
                GraphBuildProgress::Idle
            ]
            .iter()
            .cloned())
        );

        let mut expected = RepositoryGraph::new();
        expected.register_note(&note_a);
        expected.register_note(&note_b);
        expected.register_note(&note_c);

        assert_eq!(*graph.read().unwrap(), expected);
    }

    #[test]
    fn test_graph_build_progress_from_count() {
        assert!(matches!(
            GraphBuildProgress::from_count(10, 100),
            GraphBuildProgress::InProgress(0.1)
        ));
        assert!(matches!(
            GraphBuildProgress::from_count(5, 10),
            GraphBuildProgress::InProgress(0.5)
        ));
        assert!(matches!(
            GraphBuildProgress::from_count(1, 4),
            GraphBuildProgress::InProgress(0.25)
        ));
        assert!(matches!(
            GraphBuildProgress::from_count(2, 2),
            GraphBuildProgress::Idle
        ));
    }
}
