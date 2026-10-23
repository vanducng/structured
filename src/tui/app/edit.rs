use crate::api::{NewTask, Patch, Scope};
use crate::model::{Task, TaskRef};

use super::schedule::{ClockChoice, DateChoice, Spec};
use super::{App, Tone, Undo};
use crate::tui::worker::Request;

impl App {
    /// A day that is being split off its series gets a new id; hold writes until it arrives.
    pub(super) fn selected_ref(&mut self) -> Option<(usize, TaskRef)> {
        let index = self.selected_index()?;
        match self.tasks[index].task_ref() {
            Ok(task_ref) if self.detaching.iter().any(|(held, _)| held == &task_ref) => {
                self.say(Tone::Busy, "saving the last change to this day…");
                None
            }
            Ok(task_ref) => Some((index, task_ref)),
            Err(err) => {
                self.say(Tone::Bad, err.to_string());
                None
            }
        }
    }

    pub(super) fn toggle(&mut self) -> Option<Request> {
        let (index, task_ref) = self.selected_ref()?;
        let task = &mut self.tasks[index];
        let (was_completed, was_at) = (task.is_completed, task.completed_at.clone());
        let completed = !task.done();
        task.set_done(completed);
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
        let (index, task_ref) = self.selected_ref()?;
        let task = self.tasks.remove(index);
        self.clamp();
        let key = self.cache_key();
        self.touch(Undo::Delete { task, index, key });
        Some(Request::Delete { task_ref })
    }

    pub(super) fn rename(&mut self, title: String) -> Option<Request> {
        let (index, task_ref) = self.selected_ref()?;
        let previous = std::mem::replace(&mut self.tasks[index].title, title.clone());
        self.touch(Undo::Title {
            task_ref: task_ref.clone(),
            previous,
        });
        let patch = Patch {
            title: Some(title),
            ..Patch::default()
        };
        Some(Request::Edit {
            task_ref,
            patch,
            scope: Some(Scope::Series),
        })
    }

    pub(super) fn update_note(&mut self, next: String, previous: String) -> Option<Request> {
        let (index, task_ref) = self.selected_ref()?;
        self.tasks[index].note = next.clone();
        self.touch(Undo::Note {
            task_ref: task_ref.clone(),
            previous,
        });
        let patch = Patch {
            note: Some(next),
            ..Patch::default()
        };
        Some(Request::Edit {
            task_ref,
            patch,
            scope: Some(Scope::Series),
        })
    }

    pub(super) fn shift_day(&mut self, delta: i64) -> Option<Request> {
        let index = self.selected_index()?;
        let anchor = self.tasks[index]
            .day
            .as_deref()
            .and_then(crate::clock::parse_stamp)
            .unwrap_or_else(crate::clock::today);
        let day = crate::clock::stamp(anchor + chrono::Duration::days(delta));
        self.reschedule(Spec::Date(DateChoice::Day(day)))
    }

    /// Moves split one recurring day off its series; time and duration change the series.
    pub(super) fn reschedule(&mut self, spec: Spec) -> Option<Request> {
        let (index, task_ref) = self.selected_ref()?;
        let previous = self.tasks[index].clone();
        let mut patch = schedule_patch(&spec);
        if previous.day.is_none() && patch.day.is_some() {
            patch.all_day = Some(true);
        }
        let moves = matches!(spec, Spec::Date(_));
        let scope = Some(if moves { Scope::Day } else { Scope::Series });
        let detaches = moves && task_ref.is_occurrence();
        apply_spec(&mut self.tasks[index], &spec);
        let removed = leaves_view(self, &self.tasks[index]);
        if removed {
            self.tasks.remove(index);
            self.clamp();
        } else {
            self.resort();
        }
        if detaches {
            let view = self.cache_key();
            self.detaching.push((task_ref.clone(), view));
        }
        let key = self.cache_key();
        self.touch(Undo::Schedule {
            previous,
            index,
            key,
            removed,
            detaches,
        });
        Some(Request::Edit {
            task_ref,
            patch,
            scope,
        })
    }

    pub(super) fn queue_add(&mut self, title: String) -> Request {
        let day = (!self.inbox).then(|| super::day_string(self.offset));
        self.query.clear();
        self.tasks.push(blank(&title, day.clone(), self.inbox));
        self.resort();
        if let Some(pos) = self
            .visible()
            .iter()
            .rposition(|task| is_draft(task, &title))
        {
            self.select(pos);
        }
        self.touch(Undo::Add {
            title: title.clone(),
        });
        let all_day = day.is_some();
        Request::Add {
            draft: NewTask {
                title,
                day,
                all_day,
                ..NewTask::default()
            },
        }
    }

    pub(super) fn finish_write(&mut self, result: Result<Option<Task>, ()>, message: String) {
        let Some(undo) = self.undos.pop_front() else {
            return;
        };
        let held = detached_ref(&undo);
        match result {
            Ok(saved) => {
                if needs_reload(&undo) {
                    self.reload_when_idle = true;
                }
                if let Some(saved) = saved {
                    self.adopt(&undo, saved);
                }
                if self.undos.is_empty() {
                    self.say(Tone::Good, message);
                }
            }
            Err(()) => {
                if let Some(task_ref) = held {
                    self.detaching.retain(|(hold, _)| hold != &task_ref);
                }
                self.revert(undo);
                self.reload_when_idle = true;
                self.say(Tone::Bad, message);
            }
        }
        if self.undos.is_empty() && self.reload_when_idle {
            self.reload_when_idle = false;
            self.want = Some(self.cache_key());
        }
    }

    /// Takes the server's copy: a new task's id, a split day's new id, and fresh subtask ids.
    fn adopt(&mut self, undo: &Undo, saved: Task) {
        let Ok(saved_ref) = saved.task_ref() else {
            return;
        };
        match undo {
            Undo::Add { title } => {
                let draft = |task: &Task| is_draft(task, title);
                let was_selected = self.selected_task().is_some_and(draft);
                if let Some(slot) = self.tasks.iter_mut().rev().find(|task| draft(task)) {
                    *slot = saved;
                    agenda_sort(self);
                    if was_selected {
                        self.follow(Some(&saved_ref));
                    }
                    self.remember();
                }
            }
            Undo::Subtasks {
                task_ref, detaches, ..
            } => {
                let fresh = saved.subtasks.clone();
                if self.undos.is_empty() || *detaches {
                    self.patch(task_ref, |local| local.subtasks = fresh.clone());
                } else {
                    self.patch(task_ref, |local| fill_ids(&mut local.subtasks, &fresh));
                }
                if *detaches {
                    self.patch(task_ref, |local| take_identity(local, &saved));
                }
                self.clamp();
            }
            Undo::Schedule {
                previous,
                detaches: true,
                ..
            } => {
                if let Ok(old) = previous.task_ref()
                    && old != saved_ref
                {
                    self.patch(&old, |local| take_identity(local, &saved));
                }
            }
            _ => {}
        }
    }

    pub(super) fn touch(&mut self, undo: Undo) {
        self.epoch += 1;
        self.undos.push_back(undo);
        self.remember();
        self.say(Tone::Busy, "saving…");
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
                removed: true,
                ..
            }
            | Undo::Delete {
                task: previous,
                index,
                key,
            } => {
                if key == self.cache_key() {
                    let index = index.min(self.tasks.len());
                    self.tasks.insert(index, previous);
                    self.resort();
                    self.remember();
                } else if let Some(slot) = self.cache.get_mut(&key) {
                    let index = index.min(slot.len());
                    slot.insert(index, previous);
                }
            }
            Undo::Schedule {
                previous,
                removed: false,
                ..
            } => {
                if let Ok(task_ref) = previous.task_ref() {
                    self.patch(&task_ref, |task| restore_schedule(task, &previous));
                    self.resort();
                }
            }
            Undo::Title { task_ref, previous } => {
                self.patch(&task_ref, |task| task.title = previous.clone())
            }
            Undo::Note { task_ref, previous } => {
                self.patch(&task_ref, |task| task.note = previous.clone())
            }
            Undo::Subtasks {
                task_ref, previous, ..
            } => {
                let superseded = self
                    .undos
                    .iter()
                    .any(|later| matches!(later, Undo::Subtasks { task_ref: next, .. } if next == &task_ref));
                if !superseded {
                    self.patch(&task_ref, |task| task.subtasks = previous.clone());
                    self.clamp();
                }
            }
            Undo::Add { title } => {
                if let Some(index) = self.tasks.iter().rposition(|task| is_draft(task, &title)) {
                    self.tasks.remove(index);
                    self.clamp();
                    self.remember();
                }
            }
        }
    }

    pub(super) fn patch(&mut self, task_ref: &TaskRef, mut apply: impl FnMut(&mut Task)) {
        let same = |task: &&mut Task| task.task_ref().ok().as_ref() == Some(task_ref);
        if let Some(task) = self.tasks.iter_mut().find(same) {
            apply(task);
        }
        for tasks in self.cache.values_mut() {
            if let Some(task) = tasks.iter_mut().find(same) {
                apply(task);
            }
        }
    }
}

/// A task added locally that the server has not answered for yet.
fn is_draft(task: &Task, title: &str) -> bool {
    task.title == title && task.id.is_none()
}

fn agenda_sort(app: &mut App) {
    crate::agenda::sort(&mut app.tasks, app.inbox);
}

/// Gives id-less local subtasks the ids the server assigned, matching by title in order.
fn fill_ids(local: &mut [crate::model::Subtask], saved: &[crate::model::Subtask]) {
    let known: Vec<String> = local.iter().filter_map(|sub| sub.id.clone()).collect();
    let mut spare = saved
        .iter()
        .filter(|sub| sub.id.as_ref().is_some_and(|id| !known.contains(id)))
        .collect::<Vec<_>>();
    for sub in local.iter_mut().filter(|sub| sub.id.is_none()) {
        if let Some(at) = spare.iter().position(|server| server.title == sub.title) {
            sub.id = spare.remove(at).id.clone();
        }
    }
}

fn take_identity(local: &mut Task, saved: &Task) {
    local.id = saved.id.clone();
    local.recurring_id = saved.recurring_id.clone();
    local.is_recurring = saved.is_recurring;
    local.day = saved.day.clone().or(local.day.take());
}

fn restore_schedule(task: &mut Task, previous: &Task) {
    task.day = previous.day.clone();
    task.start_time = previous.start_time;
    task.duration = previous.duration;
    task.is_all_day = previous.is_all_day;
    task.is_in_inbox = previous.is_in_inbox;
}

fn detached_ref(undo: &Undo) -> Option<TaskRef> {
    match undo {
        Undo::Subtasks {
            task_ref,
            detaches: true,
            ..
        } => Some(task_ref.clone()),
        Undo::Schedule {
            previous,
            detaches: true,
            ..
        } => previous.task_ref().ok(),
        _ => None,
    }
}

/// Recurring edits change the series or split a day into a new task, so re-read the view.
fn needs_reload(undo: &Undo) -> bool {
    match undo {
        Undo::Add { .. } => true,
        Undo::Title { task_ref, .. } | Undo::Note { task_ref, .. } => task_ref.is_occurrence(),
        Undo::Schedule { previous, .. } => previous.task_ref().is_ok_and(|r| r.is_occurrence()),
        Undo::Subtasks { detaches, .. } => *detaches,
        Undo::Complete { .. } | Undo::Delete { .. } => false,
    }
}

fn schedule_patch(spec: &Spec) -> Patch {
    match spec {
        Spec::Date(DateChoice::Inbox) => Patch {
            inbox: true,
            ..Patch::default()
        },
        Spec::Date(DateChoice::Day(day)) => Patch {
            day: Some(day.clone()),
            ..Patch::default()
        },
        Spec::Clock(ClockChoice::AllDay) => Patch {
            all_day: Some(true),
            ..Patch::default()
        },
        Spec::Clock(ClockChoice::At { start, duration }) => Patch {
            start_time: Some(*start),
            duration: *duration,
            all_day: Some(false),
            ..Patch::default()
        },
        Spec::Duration(mins) => Patch {
            duration: Some(*mins),
            ..Patch::default()
        },
    }
}

fn apply_spec(task: &mut Task, spec: &Spec) {
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
        title: title.to_owned(),
        day,
        is_all_day: !inbox,
        is_in_inbox: inbox,
        ..Task::default()
    }
}
