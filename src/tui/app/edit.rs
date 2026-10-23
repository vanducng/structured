use crate::api::NewTask;
use crate::model::{Task, TaskRef};

use super::{App, Undo};
use crate::tui::worker::Request;

impl App {
    pub(super) fn toggle(&mut self) -> Option<Request> {
        let index = self.selected_index()?;
        let task_ref = match self.tasks[index].task_ref() {
            Ok(task_ref) => task_ref,
            Err(err) => {
                self.status = err.to_string();
                return None;
            }
        };
        let was_completed = self.tasks[index].is_completed;
        let was_at = self.tasks[index].completed_at.clone();
        let completed = !self.tasks[index].done();
        self.tasks[index].is_completed = completed;
        self.tasks[index].completed_at = completed.then(|| "1".into());
        self.checks.retain(|(id, _)| id != &task_ref);
        self.checks.push((task_ref.clone(), completed));
        self.touch(Undo::Complete {
            task_ref: task_ref.clone(),
            was_completed,
            was_at,
        });
        Some(Request::Complete {
            task_ref,
            completed,
        })
    }

    pub(super) fn delete_selected(&mut self) -> Option<Request> {
        let index = self.selected_index()?;
        let task_ref = match self.tasks[index].task_ref() {
            Ok(task_ref) => task_ref,
            Err(err) => {
                self.status = err.to_string();
                return None;
            }
        };
        let task = self.tasks.remove(index);
        self.clamp();
        let key = self.cache_key();
        self.touch(Undo::Delete { task, index, key });
        Some(Request::Delete { task_ref })
    }

    pub(super) fn rename(&mut self, title: String) -> Option<Request> {
        let index = self.selected_index()?;
        let task_ref = match self.tasks[index].task_ref() {
            Ok(task_ref) => task_ref,
            Err(err) => {
                self.status = err.to_string();
                return None;
            }
        };
        let previous = std::mem::replace(&mut self.tasks[index].title, title.clone());
        self.touch(Undo::Title {
            task_ref: task_ref.clone(),
            previous,
        });
        Some(Request::Title { task_ref, title })
    }

    pub(super) fn update_note(&mut self, next: String, previous: String) -> Option<Request> {
        let index = self.selected_index()?;
        let task_ref = match self.tasks[index].task_ref() {
            Ok(task_ref) => task_ref,
            Err(err) => {
                self.status = err.to_string();
                return None;
            }
        };
        self.tasks[index].note = next.clone();
        self.touch(Undo::Note {
            task_ref: task_ref.clone(),
            previous,
        });
        Some(Request::Note {
            task_ref,
            note: next,
        })
    }

    pub(super) fn shift_day(&mut self, delta: i64) -> Option<Request> {
        let index = self.selected_index()?;
        let anchor = self.tasks[index]
            .day
            .as_deref()
            .and_then(|day| chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Local::now().date_naive());
        let day = (anchor + chrono::Duration::days(delta))
            .format("%Y-%m-%d")
            .to_string();
        self.reschedule(super::schedule::Spec::Date(
            super::schedule::DateChoice::Day(day),
        ))
    }

    pub(super) fn reschedule(&mut self, spec: super::schedule::Spec) -> Option<Request> {
        let index = self.selected_index()?;
        let task_ref = match self.tasks[index].task_ref() {
            Ok(task_ref) => task_ref,
            Err(err) => {
                self.status = err.to_string();
                return None;
            }
        };
        let previous = self.tasks[index].clone();
        let patch = schedule_patch(&spec, task_ref.recurring_id.is_some());
        apply_spec(&mut self.tasks[index], &spec);
        let removed = leaves_view(self, &self.tasks[index]);
        if removed {
            self.tasks.remove(index);
            self.clamp();
        }
        let key = self.cache_key();
        self.touch(Undo::Schedule {
            previous,
            index,
            key,
            removed,
        });
        Some(Request::Schedule { task_ref, patch })
    }

    pub(super) fn queue_add(&mut self, title: String) -> Request {
        let day = if self.inbox {
            None
        } else {
            Some(super::day_string(self.offset))
        };
        self.tasks.push(blank(&title, day.clone(), self.inbox));
        self.selected = self.visible().len().saturating_sub(1);
        self.touch(Undo::Add {
            title: title.clone(),
        });
        Request::Add {
            draft: NewTask {
                title,
                note: None,
                day,
                start_time: None,
                duration: None,
                all_day: !self.inbox,
                symbol: None,
            },
        }
    }

    pub(super) fn finish_write(&mut self, ok: bool, message: String) {
        let Some(undo) = self.undos.pop_front() else {
            return;
        };
        if ok {
            if matches!(undo, Undo::Add { .. }) {
                self.reload_when_idle = true;
            }
            if self.undos.is_empty() {
                self.status = message;
            }
        } else {
            self.revert(undo);
            self.status = message;
        }
        if self.undos.is_empty() && self.reload_when_idle {
            self.reload_when_idle = false;
            self.want = Some(self.cache_key());
        }
    }

    fn touch(&mut self, undo: Undo) {
        self.epoch += 1;
        self.undos.push_back(undo);
        self.remember();
        self.status = "saving".into();
    }

    fn revert(&mut self, undo: Undo) {
        match undo {
            Undo::Complete {
                task_ref,
                was_completed,
                was_at,
            } => {
                self.checks.retain(|(id, _)| id != &task_ref);
                self.patch(&task_ref, |task| {
                    task.is_completed = was_completed;
                    task.completed_at = was_at.clone();
                });
            }
            Undo::Schedule {
                previous,
                index,
                key,
                removed,
            } => {
                if removed {
                    if key == self.cache_key() {
                        let index = index.min(self.tasks.len());
                        self.tasks.insert(index, previous);
                        self.clamp();
                        self.remember();
                    } else if let Some(slot) = self.cache.get_mut(&key) {
                        let index = index.min(slot.len());
                        slot.insert(index, previous);
                    }
                } else {
                    let found = {
                        let wanted = previous.task_ref().ok();
                        if let Some(task) = self
                            .tasks
                            .iter_mut()
                            .find(|task| task.task_ref().ok().as_ref() == wanted.as_ref())
                        {
                            *task = previous;
                            true
                        } else {
                            false
                        }
                    };
                    if found {
                        self.remember();
                    }
                }
            }
            Undo::Delete { task, index, key } => {
                if key == self.cache_key() {
                    let index = index.min(self.tasks.len());
                    self.tasks.insert(index, task.clone());
                    self.clamp();
                    self.remember();
                } else if let Some(slot) = self.cache.get_mut(&key) {
                    let index = index.min(slot.len());
                    slot.insert(index, task);
                }
            }
            Undo::Title { task_ref, previous } => {
                self.patch(&task_ref, |task| task.title = previous.clone());
            }
            Undo::Note { task_ref, previous } => {
                self.patch(&task_ref, |task| task.note = previous.clone());
            }
            Undo::Add { title } => {
                if let Some(index) = self.tasks.iter().rposition(|task| {
                    task.title == title && task.id.as_ref().is_none_or(|id| id.is_empty())
                }) {
                    self.tasks.remove(index);
                    self.clamp();
                    self.remember();
                }
            }
        }
    }

    fn patch(&mut self, task_ref: &TaskRef, mut apply: impl FnMut(&mut Task)) {
        if let Some(task) = self
            .tasks
            .iter_mut()
            .find(|task| task.task_ref().ok().as_ref() == Some(task_ref))
        {
            apply(task);
        }
        for tasks in self.cache.values_mut() {
            if let Some(task) = tasks
                .iter_mut()
                .find(|task| task.task_ref().ok().as_ref() == Some(task_ref))
            {
                apply(task);
            }
        }
    }
}

fn schedule_patch(spec: &super::schedule::Spec, recurring: bool) -> crate::api::Patch {
    use super::schedule::{ClockChoice, DateChoice, Spec};
    let mut patch = crate::api::Patch {
        title: None,
        note: None,
        day: None,
        start_time: None,
        duration: None,
        all_day: None,
        inbox: false,
        new_day: None,
    };
    match spec {
        Spec::Date(DateChoice::Inbox) => patch.inbox = true,
        Spec::Date(DateChoice::Day(day)) if recurring => patch.new_day = Some(day.clone()),
        Spec::Date(DateChoice::Day(day)) => patch.day = Some(day.clone()),
        Spec::Clock(ClockChoice::AllDay) => patch.all_day = Some(true),
        Spec::Clock(ClockChoice::At { start, duration }) => {
            patch.start_time = Some(*start);
            patch.duration = *duration;
            patch.all_day = Some(false);
        }
        Spec::Duration(mins) => patch.duration = Some(*mins),
    }
    patch
}

fn apply_spec(task: &mut Task, spec: &super::schedule::Spec) {
    use super::schedule::{ClockChoice, DateChoice, Spec};
    match spec {
        Spec::Date(DateChoice::Inbox) => {
            task.is_in_inbox = true;
            task.day = None;
        }
        Spec::Date(DateChoice::Day(day)) => {
            task.day = Some(day.clone());
            task.is_in_inbox = false;
        }
        Spec::Clock(ClockChoice::AllDay) => {
            task.is_all_day = true;
            task.start_time = None;
        }
        Spec::Clock(ClockChoice::At { start, duration }) => {
            task.is_all_day = false;
            task.start_time = Some(*start);
            if let Some(duration) = duration {
                task.duration = Some(*duration);
            }
        }
        Spec::Duration(mins) => task.duration = Some(*mins),
    }
}

fn leaves_view(app: &App, task: &Task) -> bool {
    if app.inbox {
        return !task.is_in_inbox;
    }
    task.is_in_inbox || task.day.as_deref() != Some(super::day_string(app.offset).as_str())
}

fn blank(title: &str, day: Option<String>, inbox: bool) -> Task {
    Task {
        id: None,
        recurring_id: None,
        title: title.to_owned(),
        note: String::new(),
        day,
        start_time: None,
        duration: None,
        is_all_day: !inbox,
        is_in_inbox: inbox,
        is_completed: false,
        is_recurring: false,
        completed_at: None,
        symbol: None,
        color: None,
        energy_level: None,
    }
}
