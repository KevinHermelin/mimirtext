#[derive(Clone, Debug, Default, PartialEq)]
pub enum RunningState {
    #[default]
    Running,
    Done,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    pub running_state: RunningState,
}

pub enum Message {
    Quit,
}

pub enum Command {
    None,
}

impl Model {
    pub fn update(&self, message: Message) -> (Model, Command) {
        let mut model = self.clone();
        let command = Command::None;

        match message {
            Message::Quit => model.running_state = RunningState::Done,
        }
        (model, command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_model() {
        assert_eq!(
            Model::default(),
            Model {
                running_state: RunningState::Running
            }
        )
    }

    #[test]
    fn test_quit() {
        let model = Model::default();

        assert_eq!(model.running_state, RunningState::Running);
        let (model, _) = model.update(Message::Quit);
        assert_eq!(model.running_state, RunningState::Done);
    }
}
