//! Functions for working with git repositories.

use std::{
    process::Command,
    sync::{Arc, RwLock},
};

use crate::repository::folder::FolderRepository;

#[derive(Clone, Debug, PartialEq)]
pub struct GitStatus {
    pub head_name: String,
}

pub trait Git {
    /// Returns the human readable symbolic name of HEAD.
    fn head_name(&self) -> Option<String>;

    fn get_status(&self) -> Option<GitStatus> {
        self.head_name().map(|head_name| GitStatus { head_name })
    }
}

/// An implementation of Git that uses an already-installed
/// git instance.
pub struct GitShell {
    repo: Arc<RwLock<FolderRepository>>,
}

impl GitShell {
    pub fn new(repo: Arc<RwLock<FolderRepository>>) -> Self {
        GitShell { repo }
    }
}

impl Git for GitShell {
    fn head_name(&self) -> Option<String> {
        let repo = self.repo.read().expect("Should be able to read repo");

        let mut command = Command::new("git");
        let output = command
            .current_dir(&repo.root)
            .args(vec!["name-rev", "--name-only", "HEAD"])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                Some(String::from_utf8(output.stdout).expect("Git output should be valid utf8"))
            } else {
                None
            }
        } else {
            None
        }
    }
}
