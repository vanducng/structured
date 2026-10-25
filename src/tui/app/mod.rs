mod card;
mod card_keys;
#[cfg(test)]
mod card_tests;
mod edit;
mod editor;
mod input;
mod keys;
mod schedule;
mod subtasks;
#[cfg(test)]
mod tests;

use std::cell::Cell;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

pub(in crate::tui) use card::{Card, Field};
pub(in crate::tui) use keys::handle;

use crate::agenda::{self, Clock, Layout};
use crate::clock;
use crate::config::{self, UiConfig};
use crate::model::{Subtask, Task, TaskRef};

use super::worker::{CacheKey, Reply, Request};

pub(in crate::tui) enum Mode {
    Normal,
    Help,
    Confirm(Doomed),
    Filter,
    Input {
        input: tui_input::Input,
        prompt: Prompt,
    },
    Card(Card),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::tui) enum Doomed {
    Task,
    Subtask,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::tui) enum Prompt {
    NewTitle,
    Day,
    Time,
    Duration,
}

impl Prompt {
    pub(in crate::tui) fn label(self) -> &'static str {
        match self {
            Self::NewTitle => "New task",
            Self::Day => "Date",
            Self::Time => "Time",
            Self::Duration => "Duration",
        }
    }

    pub(in crate::tui) fn hint(self) -> &'static str {
        match self {
            Self::NewTitle => "type a title",
            Self::Day => "YYYY-MM-DD, today, +1, or inbox",
            Self::Time => "09:30, 09:30 45, or empty for all day",
            Self::Duration => "45, 1h30, or 1:30",
        }
    }

    pub(in crate::tui) fn steps(self) -> bool {
        matches!(self, Self::Day | Self::Time | Self::Duration)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::tui) enum Focus {
    List,
    Subtasks,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::tui) enum Tone {
    Quiet,
    Busy,
    Good,
    Bad,
}

pub(in crate::tui) struct Status {
    pub(in crate::tui) text: String,
    pub(in crate::tui) tone: Tone,
    at: Instant,
}

enum Undo {
    Complete {
        task_ref: TaskRef,
        was_completed: bool,
        was_at: Option<String>,
    },
    Delete {
        task: Task,
        index: usize,
        key: CacheKey,
    },
    Title {
        task_ref: TaskRef,
        previous: String,
    },
    Note {
        task_ref: TaskRef,
        previous: String,
    },
    Add {
        title: String,
    },
    Schedule {
        previous: Task,
        index: usize,
        key: CacheKey,
        removed: bool,
        detaches: bool,
    },
    Subtasks {
        task_ref: TaskRef,
        previous: Vec<Subtask>,
        detaches: bool,
    },
}

pub(in crate::tui) struct App {
    pub(in crate::tui) inbox: bool,
    pub(in crate::tui) window: bool,
    pub(in crate::tui) offset: i64,
    tasks: Vec<Task>,
    pub(in crate::tui) selected: usize,
    pub(in crate::tui) sub_selected: usize,
    pub(in crate::tui) focus: Focus,
    pub(in crate::tui) status: Status,
    pub(in crate::tui) mode: Mode,
    g_pending: bool,
    pub(in crate::tui) query: String,
    pub(in crate::tui) detail_scroll: u16,
    pub(in crate::tui) list_offset: Cell<usize>,
    /// The details pane's inner width at the last draw, so up and down can walk wrapped rows.
    pub(in crate::tui) detail_width: Cell<usize>,
    pub(in crate::tui) config: UiConfig,
    cache: HashMap<CacheKey, Vec<Task>>,
    want: Option<CacheKey>,
    prefetch: Option<CacheKey>,
    load_inflight: bool,
    pub(in crate::tui) loaded: bool,
    epoch: u64,
    undos: VecDeque<Undo>,
    reload_when_idle: bool,
    checks: Vec<(TaskRef, bool)>,
    detaching: Vec<(TaskRef, CacheKey)>,
    pub(in crate::tui) quitting: bool,
}

impl App {
    pub(in crate::tui) fn new(config: UiConfig) -> Self {
        Self {
            inbox: false,
            window: config.window,
            offset: 0,
            tasks: Vec::new(),
            selected: 0,
            sub_selected: 0,
            focus: Focus::List,
            status: Status {
                text: "loading…".into(),
                tone: Tone::Busy,
                at: Instant::now(),
            },
            mode: Mode::Normal,
            g_pending: false,
            query: String::new(),
            detail_scroll: 0,
            list_offset: Cell::new(0),
            detail_width: Cell::new(0),
            config,
            cache: HashMap::new(),
            want: None,
            prefetch: None,
            load_inflight: false,
            loaded: false,
            epoch: 0,
            undos: VecDeque::new(),
            reload_when_idle: false,
            checks: Vec::new(),
            detaching: Vec::new(),
            quitting: false,
        }
    }

    pub(in crate::tui) fn pending(&self) -> usize {
        self.undos.len()
    }

    /// The configured zone the screen reads in, or None when the system zone is in charge.
    pub(in crate::tui) fn zone_label(&self) -> Option<String> {
        let tz = clock::zone()?;
        (!self.config.zones.is_empty()).then(|| config::label(&self.config.zones, tz))
    }

    /// Days and times come back from the server already converted, so a new zone means a fresh load.
    pub(in crate::tui) fn next_zone(&mut self) {
        let current = clock::zone();
        let Some(zone) = config::next_zone(&self.config.zones, current).cloned() else {
            self.say(Tone::Quiet, "add timezones to config.toml to switch zones");
            return;
        };
        if Some(zone.tz) == current {
            self.say(Tone::Quiet, "list a second zone in config.toml to switch");
            return;
        }
        if !self.undos.is_empty() {
            self.say(Tone::Busy, "still saving, switch zones when it finishes");
            return;
        }
        clock::set_zone(Some(zone.tz));
        self.epoch += 1;
        self.cache.clear();
        self.prefetch = None;
        self.checks.clear();
        self.detaching.clear();
        self.reveal();
        self.say(Tone::Quiet, format!("times in {}", zone.label));
    }

    /// Keeps the highlighted task when the search changes, if it still matches.
    pub(in crate::tui) fn set_query(&mut self, query: String) {
        let selected = self.selected_task().and_then(|task| task.task_ref().ok());
        self.query = query;
        self.selected = 0;
        self.follow(selected.as_ref());
        self.detail_scroll = 0;
    }

    pub(in crate::tui) fn clear_query(&mut self) {
        self.set_query(String::new());
    }

    /// Why a write did not go out, for a draft that stays open.
    fn refusal(&self) -> String {
        if self.status.text.is_empty() {
            "not saved, try again".into()
        } else {
            self.status.text.clone()
        }
    }

    pub(in crate::tui) fn say(&mut self, tone: Tone, text: impl Into<String>) {
        self.status = Status {
            text: text.into(),
            tone,
            at: Instant::now(),
        };
    }

    /// Good and quiet notices fade so the footer does not keep stale news; errors stay until the next action.
    pub(in crate::tui) fn expire_status(&mut self) -> bool {
        let stale = matches!(self.status.tone, Tone::Good | Tone::Quiet)
            && !self.status.text.is_empty()
            && self.status.at.elapsed() > Duration::from_secs(4);
        if stale {
            self.say(Tone::Quiet, "");
        }
        stale
    }

    pub(in crate::tui) fn visible(&self) -> Vec<&Task> {
        let clock = Clock::now();
        self.tasks
            .iter()
            .filter(|task| self.shows(task, &clock))
            .collect()
    }

    fn shows(&self, task: &Task, clock: &Clock) -> bool {
        matches(task, &self.query) && (self.layout() != Layout::Window || clock.in_window(task))
    }

    /// Today reads as the calendar day or the rolling 24 hours, per `view` in config.toml and `w`.
    pub(in crate::tui) fn layout(&self) -> Layout {
        match (self.inbox, self.offset) {
            (true, _) => Layout::Inbox,
            (false, 0) if self.window => Layout::Window,
            (false, offset) => Layout::Day {
                now_line: offset == 0 && self.query.is_empty(),
            },
        }
    }

    pub(in crate::tui) fn selected_task(&self) -> Option<&Task> {
        self.selected_index().map(|index| &self.tasks[index])
    }

    /// An open card stays on its own task, even when the list under it shifts or filters it out.
    fn selected_index(&self) -> Option<usize> {
        if let Mode::Card(card) = &self.mode {
            return self
                .tasks
                .iter()
                .position(|task| task.task_ref().ok().as_ref() == Some(&card.task_ref));
        }
        let clock = Clock::now();
        self.tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| self.shows(task, &clock))
            .nth(self.selected)
            .map(|(index, _)| index)
    }

    pub(in crate::tui) fn viewed_day(&self) -> chrono::NaiveDate {
        clock::today() + chrono::Duration::days(self.offset)
    }

    pub(in crate::tui) fn inbox_open(&self) -> Option<usize> {
        let tasks = if self.inbox {
            Some(&self.tasks)
        } else {
            self.cache.get(&CacheKey::Inbox)
        };
        tasks.map(|tasks| tasks.iter().filter(|task| !task.done()).count())
    }

    fn cache_key(&self) -> CacheKey {
        if self.inbox {
            CacheKey::Inbox
        } else if self.offset == 0 && self.window {
            CacheKey::Now
        } else if self.offset == 0 {
            CacheKey::Today
        } else {
            CacheKey::Day(day_string(self.offset))
        }
    }

    fn clamp(&mut self) {
        let count = self.visible().len();
        self.selected = self.selected.min(count.saturating_sub(1));
        let subs = self.selected_task().map_or(0, |task| task.subtasks.len());
        self.sub_selected = self.sub_selected.min(subs.saturating_sub(1));
        if count == 0 {
            self.focus = Focus::List;
        }
    }

    pub(in crate::tui) fn select(&mut self, index: usize) {
        if index != self.selected {
            self.sub_selected = 0;
        }
        self.selected = index;
        self.detail_scroll = 0;
    }

    pub(in crate::tui) fn reveal(&mut self) {
        let key = self.cache_key();
        self.select(0);
        self.focus = Focus::List;
        self.list_offset.set(0);
        if let Some(tasks) = self.cache.get(&key).cloned() {
            self.tasks = tasks;
            self.loaded = true;
            self.say(Tone::Quiet, "");
        } else {
            self.tasks.clear();
            self.loaded = false;
            self.say(Tone::Busy, "loading…");
        }
        self.want = Some(key);
    }

    pub(in crate::tui) fn arm_refresh(&mut self) {
        self.checks.clear();
        self.want = Some(self.cache_key());
    }

    pub(in crate::tui) fn should_poll_refresh(&self) -> bool {
        matches!(self.mode, Mode::Normal)
            && self.want.is_none()
            && !self.load_inflight
            && self.undos.is_empty()
    }

    pub(in crate::tui) fn pump(&mut self, tx: &Sender<Request>) {
        if self.load_inflight {
            return;
        }
        let (key, show) = if let Some(key) = self.want.take() {
            (key, true)
        } else if self.undos.is_empty() {
            let Some(key) = self.prefetch.take() else {
                return;
            };
            (key, false)
        } else {
            return;
        };
        self.load_inflight = true;
        let _ = tx.send(Request::Load {
            epoch: self.epoch,
            key,
            show,
        });
    }

    pub(in crate::tui) fn apply_reply(&mut self, reply: Reply) {
        match reply {
            Reply::Loaded {
                epoch,
                key,
                show,
                tasks,
            } => self.apply_load(epoch, key, tasks, show),
            Reply::LoadFailed { key, show, message } => self.fail_load(key, show, message),
            Reply::Saved { message, task } => {
                self.finish_write(Ok(task.map(|task| *task)), message.into())
            }
            Reply::WriteFailed { message } => self.finish_write(Err(()), message),
        }
        self.settle_card();
    }

    fn apply_load(&mut self, epoch: u64, key: CacheKey, mut tasks: Vec<Task>, show: bool) {
        self.load_inflight = false;
        agenda::sort(&mut tasks, key == CacheKey::Inbox);
        if epoch != self.epoch {
            if key == self.cache_key() {
                if self.undos.is_empty() {
                    self.want = Some(key);
                } else {
                    self.reload_when_idle = true;
                }
            }
            return;
        }
        if show && key == self.cache_key() {
            if self.undos.is_empty() {
                self.detaching.retain(|(_, view)| view != &key);
            }
            self.replace_tasks(tasks);
            self.remember();
            self.loaded = true;
            if self.undos.is_empty() && self.status.tone == Tone::Busy {
                self.say(Tone::Quiet, "");
            }
        } else {
            self.cache.insert(key, tasks);
        }
        if show {
            self.consider_prefetch();
        }
    }

    fn fail_load(&mut self, key: CacheKey, show: bool, message: String) {
        self.load_inflight = false;
        if show && key == self.cache_key() {
            self.say(Tone::Bad, message);
        }
    }

    fn consider_prefetch(&mut self) {
        if self.want.is_some() || self.prefetch.is_some() || !self.undos.is_empty() {
            return;
        }
        let other = match (self.inbox, self.window) {
            (true, true) => CacheKey::Now,
            (true, false) => CacheKey::Today,
            (false, _) => CacheKey::Inbox,
        };
        if !self.cache.contains_key(&other) {
            self.prefetch = Some(other);
        }
    }

    fn replace_tasks(&mut self, tasks: Vec<Task>) {
        let selected = self.selected_task().and_then(|task| task.task_ref().ok());
        self.tasks = tasks;
        self.apply_checks();
        self.follow(selected.as_ref());
    }

    fn follow(&mut self, task_ref: Option<&TaskRef>) {
        if let Some(task_ref) = task_ref
            && let Some(pos) = self
                .visible()
                .iter()
                .position(|task| task.task_ref().ok().as_ref() == Some(task_ref))
        {
            self.selected = pos;
        }
        self.clamp();
    }

    fn resort(&mut self) {
        let selected = self.selected_task().and_then(|task| task.task_ref().ok());
        agenda::sort(&mut self.tasks, self.inbox);
        self.follow(selected.as_ref());
    }

    fn apply_checks(&mut self) {
        let checks = std::mem::take(&mut self.checks);
        let mut keep = Vec::new();
        for (task_ref, completed) in checks {
            let Some(task) = self
                .tasks
                .iter_mut()
                .find(|task| task.task_ref().ok().as_ref() == Some(&task_ref))
            else {
                keep.push((task_ref, completed));
                continue;
            };
            if task.done() == completed {
                continue;
            }
            task.set_done(completed);
            keep.push((task_ref, completed));
        }
        self.checks = keep;
    }

    fn remember(&mut self) {
        let key = self.cache_key();
        self.cache.insert(key, self.tasks.clone());
    }
}

pub(in crate::tui) fn day_string(offset: i64) -> String {
    clock::stamp(clock::today() + chrono::Duration::days(offset))
}

pub(in crate::tui) fn matches(task: &Task, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let query = query.to_lowercase();
    task.title.to_lowercase().contains(&query)
        || task.note.to_lowercase().contains(&query)
        || task
            .subtasks
            .iter()
            .any(|sub| sub.title.to_lowercase().contains(&query))
}
