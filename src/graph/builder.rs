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
///
/// First index is the number of files indexed.
/// Second index is the total number of files to index.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphBuildProgress(pub usize, pub usize);

impl GraphBuildProgress {
    /// Calculates percentage done, as a float between 0.0 and 1.0.
    pub fn percentage(&self) -> f32 {
        let GraphBuildProgress(done, total) = self;
        let done: f32 = *done as f32;
        let total: f32 = *total as f32;
        done / total
    }
    /// `true` if the job is complete, i.e. all files have been indexed.
    pub fn done(&self) -> bool {
        let GraphBuildProgress(done, total) = self;
        done == total
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
            .send(GraphBuildProgress(i + 1, notes_count))
            .expect("should be able to send graph progress");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_build_progress_percentage() {
        assert_eq!(GraphBuildProgress(10, 100).percentage(), 0.1);
        assert_eq!(GraphBuildProgress(5, 10).percentage(), 0.5);
        assert_eq!(GraphBuildProgress(1, 4).percentage(), 0.25);
    }

    #[test]
    fn test_graph_build_progress_done() {
        assert!(!GraphBuildProgress(10, 100).done());
        assert!(!GraphBuildProgress(0, 5).done());
        assert!(!GraphBuildProgress(99, 100).done());
        assert!(GraphBuildProgress(10, 10).done());
    }
}
