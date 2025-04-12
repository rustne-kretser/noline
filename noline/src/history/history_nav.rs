use super::{CircularSlice, History};

/// Wrapper used for history navigation in [`core::Line`]
pub(crate) struct HistoryNavigator<'a, H: History> {
    pub(crate) history: &'a mut H,
    position: Option<usize>,
}

impl<'a, H: History> HistoryNavigator<'a, H> {
    pub(crate) fn new(history: &'a mut H) -> Self {
        Self {
            history,
            position: None,
        }
    }

    fn set_position(&mut self, position: usize) -> usize {
        *self.position.insert(position)
    }

    fn get_position(&mut self) -> usize {
        *self
            .position
            .get_or_insert_with(|| self.history.number_of_entries())
    }

    pub(crate) fn move_up(&mut self) -> Result<CircularSlice<'_>, ()> {
        let position = self.get_position();

        if position > 0 {
            let position = self.set_position(position - 1);

            Ok(self.history.get_entry(position).unwrap())
        } else {
            Err(())
        }
    }

    pub(crate) fn move_down(&mut self) -> Result<CircularSlice<'_>, ()> {
        let new_position = self.get_position() + 1;

        if new_position < self.history.number_of_entries() {
            let position = self.set_position(new_position);

            Ok(self.history.get_entry(position).unwrap())
        } else {
            Err(())
        }
    }

    pub(crate) fn reset(&mut self) {
        self.position = None;
    }

    pub(crate) fn is_active(&self) -> bool {
        self.position.is_some()
    }
}
