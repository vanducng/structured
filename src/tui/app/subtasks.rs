use crate::model::Subtask;

use super::{App, Focus, Tone, Undo};
use crate::tui::worker::Request;

impl App {
    pub(in crate::tui) fn subtask_count(&self) -> usize {
        self.selected_task().map_or(0, |task| task.subtasks.len())
    }

    pub(super) fn focus_subtasks(&mut self) -> bool {
        if self.selected_task().is_none() {
            self.say(Tone::Bad, "no task selected");
            return false;
        }
        self.focus = Focus::Subtasks;
        self.sub_selected = self
            .sub_selected
            .min(self.subtask_count().saturating_sub(1));
        true
    }

    pub(super) fn move_subtask(&mut self, delta: isize) {
        let count = self.subtask_count();
        if count > 0 {
            self.sub_selected = self
                .sub_selected
                .saturating_add_signed(delta)
                .min(count - 1);
        }
    }

    pub(super) fn toggle_subtask(&mut self) -> Option<Request> {
        let pick = self.sub_selected;
        let pending = self
            .selected_task()
            .and_then(|task| task.subtasks.get(pick))
            .is_some_and(|sub| sub.id.is_none());
        if pending {
            self.say(Tone::Busy, "still saving that subtask…");
            return None;
        }
        self.change_subtasks(false, |subs| {
            let sub = subs.get_mut(pick)?;
            let done = !sub.done();
            sub.set_done(done);
            Some(())
        })
    }

    pub(super) fn add_subtask(&mut self, at: usize, title: String) -> Option<Request> {
        let request = self.change_subtasks(true, |subs| {
            subs.insert(at.min(subs.len()), Subtask::new(&title));
            Some(())
        });
        if request.is_some() {
            self.sub_selected = at.min(self.subtask_count().saturating_sub(1));
        }
        request
    }

    pub(super) fn rename_subtask(&mut self, pick: usize, title: String) -> Option<Request> {
        self.change_subtasks(true, |subs| {
            subs.get_mut(pick)?.title = title;
            Some(())
        })
    }

    pub(super) fn delete_subtask(&mut self) -> Option<Request> {
        let pick = self.sub_selected;
        let request = self.change_subtasks(true, |subs| {
            (pick < subs.len()).then(|| {
                subs.remove(pick);
            })
        });
        self.clamp();
        request
    }

    pub(super) fn reorder_subtask(&mut self, delta: isize) -> Option<Request> {
        let from = self.sub_selected;
        let to = from
            .checked_add_signed(delta)
            .filter(|to| *to < self.subtask_count())?;
        let request = self.change_subtasks(true, |subs| {
            subs.swap(from, to);
            Some(())
        });
        if request.is_some() {
            self.sub_selected = to;
        }
        request
    }

    fn change_subtasks(
        &mut self,
        structural: bool,
        change: impl FnOnce(&mut Vec<Subtask>) -> Option<()>,
    ) -> Option<Request> {
        let (index, task_ref) = self.selected_ref()?;
        let previous = self.tasks[index].subtasks.clone();
        if change(&mut self.tasks[index].subtasks).is_none() {
            self.say(Tone::Bad, "no subtask selected");
            return None;
        }
        let subtasks = self.tasks[index].subtasks.clone();
        let detaches = structural && task_ref.is_occurrence();
        if detaches {
            let view = self.cache_key();
            self.detaching.push((task_ref.clone(), view));
        }
        self.touch(Undo::Subtasks {
            task_ref: task_ref.clone(),
            previous,
            detaches,
        });
        Some(Request::Subtasks {
            task_ref,
            subtasks,
            structural,
        })
    }
}
