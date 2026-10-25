use tui_input::Input;

use super::{App, Mode, Tone};
use crate::model::{Subtask, TaskRef};

/// An editable spot on the details card. `NewSubtask(at)` is a draft row that lands at index `at`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::tui) enum Field {
    Title,
    Subtask(usize),
    NewSubtask(usize),
    Note,
}

impl Field {
    pub(in crate::tui) fn label(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Subtask(_) => "Subtask",
            Self::NewSubtask(_) => "New subtask",
            Self::Note => "Note",
        }
    }

    /// The wrap width in a details pane `pane` columns wide. Subtask rows sit behind the six-column `  › ☐ ` lead.
    pub(in crate::tui) fn width(self, pane: usize) -> usize {
        match self {
            Self::Title | Self::Note => pane.saturating_sub(4),
            Self::Subtask(_) | Self::NewSubtask(_) => pane.saturating_sub(7),
        }
    }

    /// On a recurring task a title or note edit reaches every repeat; a subtask edit stays on the day.
    pub(in crate::tui) fn series(self) -> bool {
        matches!(self, Self::Title | Self::Note)
    }
}

/// The card holds its task by ref, so a list that shifts under it cannot send a save elsewhere.
pub(in crate::tui) struct Card {
    pub(in crate::tui) field: Field,
    pub(in crate::tui) input: Input,
    pub(super) original: String,
    pub(super) task_ref: TaskRef,
    /// The row being renamed, or the row above a draft row, found again after every reply.
    anchor: Option<Subtask>,
}

impl App {
    pub(super) fn open_card(&mut self, field: Field) {
        if let Some(card) = self.card_at(field) {
            self.mode = Mode::Card(card);
        }
    }

    /// While a card is open, `selected_task` is the card's own task.
    pub(super) fn card_at(&mut self, field: Field) -> Option<Card> {
        let Some(task) = self.selected_task() else {
            self.say(Tone::Bad, "no task selected");
            return None;
        };
        let Ok(task_ref) = task.task_ref() else {
            self.say(Tone::Busy, "still saving this task…");
            return None;
        };
        let count = task.subtasks.len();
        let field = match field {
            Field::Subtask(pick) if pick >= count => Field::NewSubtask(count),
            Field::NewSubtask(at) => Field::NewSubtask(at.min(count)),
            field => field,
        };
        let original = match field {
            Field::Title => task.title.clone(),
            Field::Subtask(pick) => task.subtasks[pick].title.clone(),
            Field::NewSubtask(_) => String::new(),
            Field::Note => task.note.trim_end_matches('\n').to_owned(),
        };
        let anchor = match field {
            Field::Subtask(pick) => task.subtasks.get(pick).cloned(),
            Field::NewSubtask(at) => at
                .checked_sub(1)
                .and_then(|above| task.subtasks.get(above))
                .cloned(),
            _ => None,
        };
        if let Field::Subtask(pick) = field {
            self.sub_selected = pick;
        }
        Some(Card {
            field,
            input: Input::new(original.clone()),
            original,
            task_ref,
            anchor,
        })
    }

    pub(super) fn close_card(&mut self) {
        if let Mode::Card(card) = std::mem::replace(&mut self.mode, Mode::Normal) {
            self.follow(Some(&card.task_ref));
        }
    }

    /// A day split off its series takes a new id, and the open card goes with it.
    pub(super) fn repin_card(&mut self, old: &TaskRef, new: &TaskRef) {
        if let Mode::Card(card) = &mut self.mode
            && &card.task_ref == old
        {
            card.task_ref = new.clone();
        }
    }

    /// A reply can drop the task or add, move, and undo rows; this puts the card back on its own row.
    pub(super) fn settle_card(&mut self) {
        let Mode::Card(card) = &self.mode else {
            return;
        };
        let Some(task) = self.selected_task() else {
            self.mode = Mode::Normal;
            self.say(Tone::Bad, "the task left this view, edit closed");
            return;
        };
        let subs = &task.subtasks;
        let near = match card.field {
            Field::Subtask(pick) => pick,
            Field::NewSubtask(at) => at.saturating_sub(1),
            _ => 0,
        };
        let found = card.anchor.as_ref().and_then(|row| row_of(subs, near, row));
        let field = match (card.field, found) {
            (Field::Subtask(_), Some(at)) => Field::Subtask(at),
            (Field::Subtask(pick), None) => Field::NewSubtask(pick.min(subs.len())),
            (Field::NewSubtask(_), Some(above)) => Field::NewSubtask(above + 1),
            (Field::NewSubtask(at), None) => Field::NewSubtask(at.min(subs.len())),
            (field, _) => field,
        };
        let task_ref = card.task_ref.clone();
        let gone = matches!(
            (card.field, field),
            (Field::Subtask(_), Field::NewSubtask(_))
        );
        let typed = card.input.value() != card.original;
        if let Mode::Card(card) = &mut self.mode {
            card.field = field;
            if gone {
                card.original.clear();
                card.anchor = None;
                if !typed {
                    card.input = Input::default();
                }
            }
        }
        if gone && typed {
            self.say(Tone::Bad, "that subtask is gone, saving adds it back");
        }
        self.follow(Some(&task_ref));
        if let Field::Subtask(pick) = field {
            self.sub_selected = pick;
        }
    }
}

/// Finds the row by id, else by title nearest to where it was: a row still saving has no id yet,
/// and a day split off its series may come back with new ids.
fn row_of(subs: &[Subtask], near: usize, row: &Subtask) -> Option<usize> {
    row.id
        .as_ref()
        .and_then(|id| subs.iter().position(|sub| sub.id.as_ref() == Some(id)))
        .or_else(|| {
            (0..subs.len())
                .filter(|&at| subs[at].title == row.title)
                .min_by_key(|&at| at.abs_diff(near))
        })
}
