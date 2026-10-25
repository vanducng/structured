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
    row_id: Option<String>,
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
        let (original, row_id) = match field {
            Field::Title => (task.title.clone(), None),
            Field::Subtask(pick) => {
                let sub = &task.subtasks[pick];
                (sub.title.clone(), sub.id.clone())
            }
            Field::NewSubtask(_) => (String::new(), None),
            Field::Note => (task.note.trim_end_matches('\n').to_owned(), None),
        };
        if let Field::Subtask(pick) = field {
            self.sub_selected = pick;
        }
        Some(Card {
            field,
            input: Input::new(original.clone()),
            original,
            task_ref,
            row_id,
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
        let field = match card.field {
            Field::Subtask(pick) => row_of(subs, pick, card.row_id.as_deref(), &card.original)
                .map_or(Field::NewSubtask(pick.min(subs.len())), Field::Subtask),
            Field::NewSubtask(at) => Field::NewSubtask(at.min(subs.len())),
            field => field,
        };
        let gone = matches!(
            (card.field, field),
            (Field::Subtask(_), Field::NewSubtask(_))
        );
        let typed = card.input.value() != card.original;
        if let Mode::Card(card) = &mut self.mode {
            card.field = field;
            if gone {
                card.original.clear();
                card.row_id = None;
                if !typed {
                    card.input = Input::default();
                }
            }
        }
        if gone && typed {
            self.say(Tone::Bad, "that subtask is gone, saving adds it back");
        }
    }
}

/// Finds the row by id once it has one, else by title nearest to where it was.
fn row_of(subs: &[Subtask], pick: usize, id: Option<&str>, title: &str) -> Option<usize> {
    match id {
        Some(id) => subs.iter().position(|sub| sub.id.as_deref() == Some(id)),
        None => (0..subs.len())
            .filter(|&at| subs[at].title == title)
            .min_by_key(|&at| at.abs_diff(pick)),
    }
}
